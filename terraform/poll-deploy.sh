#!/bin/bash
# poll-deploy.sh: Lightweight agent to pull new image when OCI tag changes
export PATH=/usr/sbin:/usr/bin:/sbin:/bin

# Metadata URL for OCI (requires Auth Header for v2)
METADATA_URL="http://169.254.169.254/opc/v2/instance/"

# Path to state file
STATE_FILE="/home/opc/.last_deployed_image"

# Fetch latest deployment data from instance metadata.
# Fall back to defined tags so older instances continue to work.
METADATA_JSON=$(curl -s -H "Authorization: Bearer Oracle" "$METADATA_URL")
TAGS=$(echo "$METADATA_JSON" | jq -r '.metadata.deployed_image // .definedTags.Operations.deployed_image // empty')
SECRETS_BASE64=$(echo "$METADATA_JSON" | jq -r '.metadata.app_secrets // .definedTags.Operations.app_secrets // empty')

if [ -z "$TAGS" ] || [ "$TAGS" == "null" ] || [ "$TAGS" == "initial" ]; then
    echo "No valid image reference found in metadata tags."
    exit 0
fi

# Compare with last deployed or check for secret updates
LAST_IMAGE=""
if [ -f "$STATE_FILE" ]; then
    LAST_IMAGE=$(cat "$STATE_FILE")
fi

# Update secrets if present
if [ -n "$SECRETS_BASE64" ] && [ "$SECRETS_BASE64" != "null" ]; then
    echo "Updating environment file from metadata..."
    echo "$SECRETS_BASE64" | base64 -d > /home/opc/app.env
    chmod 600 /home/opc/app.env
    chown opc:opc /home/opc/app.env
fi

if [ "$TAGS" != "$LAST_IMAGE" ]; then
    echo "New deployment detected: $TAGS"

    GHCR_USERNAME_VALUE=$(grep '^GHCR_USERNAME=' /home/opc/app.env 2>/dev/null | cut -d'=' -f2-)
    GHCR_TOKEN_VALUE=$(grep '^GHCR_TOKEN=' /home/opc/app.env 2>/dev/null | cut -d'=' -f2-)
    if [ -n "$GHCR_USERNAME_VALUE" ] && [ -n "$GHCR_TOKEN_VALUE" ]; then
        echo "$GHCR_TOKEN_VALUE" | docker login ghcr.io -u "$GHCR_USERNAME_VALUE" --password-stdin
    fi

    if ! docker pull "$TAGS"; then
        echo "Failed to pull image $TAGS"
        exit 1
    fi

    # 2. Blue/Green style swap (simple version)
    echo "Swapping containers..."
    docker stop deductible-app-old || true
    docker rename deductible-app deductible-app-old || true
    
    docker run -d \
      --name deductible-app \
      --restart always \
      --network=host \
      --env-file /home/opc/app.env \
      --cap-drop=ALL \
      --security-opt no-new-privileges \
      "$TAGS"
      
    # Check if started
    sleep 5
    if docker ps | grep -q deductible-app; then
        echo "Successfully deployed $TAGS"
        echo "$TAGS" > "$STATE_FILE"
        docker rm -f deductible-app-old || true

        SITE_DOMAIN=$(grep SITE_DOMAIN /home/opc/app.env | cut -d'=' -f2)
        if [ -n "$SITE_DOMAIN" ]; then
            echo "Setting up Caddy for $SITE_DOMAIN"
            docker rm -f caddy || true
            printf '%s\n' "${SITE_DOMAIN} {" '  reverse_proxy localhost:8080' '}' > /home/opc/Caddyfile

            docker run -d --name caddy --restart unless-stopped --network host \
              -v /home/opc/Caddyfile:/etc/caddy/Caddyfile:z \
              -v caddy_data:/data -v caddy_config:/config docker.io/library/caddy:2
        fi
    else
        echo "New container failed to start. Rolling back..."
        docker start deductible-app-old || true
        docker rename deductible-app-old deductible-app || true
        exit 1
    fi
fi
