#!/bin/bash
# poll-deploy.sh: Lightweight agent to pull new image when OCI tag changes
set -euo pipefail
export PATH=/usr/sbin:/usr/bin:/sbin:/bin

# Metadata URL for OCI (requires Auth Header for v2)
METADATA_URL="http://169.254.169.254/opc/v2/instance/"

# Path to state file
STATE_FILE="/home/opc/.last_deployed_image"
REPO_URL="https://github.com/deductible-tracker/deductible-tracker.git"
REPO_DIR="/home/opc/deductible-tracker-src"
APP_ENV="/home/opc/app.env"

echo "Checking for new deployment at $(date)..."

# Fetch latest deployment data from instance metadata.
# Fall back to defined tags so older instances continue to work.
METADATA_JSON=$(curl -s -f -H "Authorization: Bearer Oracle" "$METADATA_URL") || {
    echo "Error: Failed to fetch metadata from $METADATA_URL" >&2
    exit 1
}

TAGS=$(echo "$METADATA_JSON" | jq -r '.metadata.deployed_image // .definedTags.Operations.deployed_image // empty')
SECRETS_BASE64=$(echo "$METADATA_JSON" | jq -r '.metadata.app_secrets // .definedTags.Operations.app_secrets // empty')

if [ -z "$TAGS" ] || [ "$TAGS" == "null" ] || [ "$TAGS" == "initial" ]; then
    echo "No valid image reference found in metadata tags."
    exit 0
fi

# Update secrets if present
if [ -n "$SECRETS_BASE64" ] && [ "$SECRETS_BASE64" != "null" ]; then
    echo "Updating environment file from metadata..."
    echo "$SECRETS_BASE64" | base64 -d > "$APP_ENV"
    chmod 600 "$APP_ENV"
    chown opc:opc "$APP_ENV"
    /usr/local/bin/configure-newrelic-infra.sh || true
fi

# Compare with last deployed
LAST_IMAGE=""
if [ -f "$STATE_FILE" ]; then
    LAST_IMAGE=$(cat "$STATE_FILE")
fi

if [ "$TAGS" == "$LAST_IMAGE" ]; then
    echo "No change in image ($TAGS). Exiting."
    exit 0
fi

echo "New deployment detected: $TAGS (Previous: $LAST_IMAGE)"

# Login to GHCR if credentials exist
if [ -f "$APP_ENV" ]; then
    GHCR_USERNAME_VALUE=$(grep '^GHCR_USERNAME=' "$APP_ENV" | cut -d'=' -f2- || true)
    GHCR_TOKEN_VALUE=$(grep '^GHCR_TOKEN=' "$APP_ENV" | cut -d'=' -f2- || true)
    if [ -n "$GHCR_USERNAME_VALUE" ] && [ -n "$GHCR_TOKEN_VALUE" ]; then
        echo "Logging into ghcr.io..."
        echo "$GHCR_TOKEN_VALUE" | docker login ghcr.io -u "$GHCR_USERNAME_VALUE" --password-stdin
    fi
fi

# 1. Pull the new image
echo "Pulling image $TAGS..."
if ! docker pull "$TAGS"; then
    echo "Failed to pull image $TAGS; attempting source build fallback"
    # Fallback to building from source if pull fails (e.g. registry issue)
    revision="${TAGS##*:}"
    if [ ! -d "$REPO_DIR/.git" ]; then
        git clone "$REPO_URL" "$REPO_DIR" || exit 1
    fi
    git -C "$REPO_DIR" fetch --depth 1 origin "$revision" || exit 1
    git -C "$REPO_DIR" checkout --force "$revision" || exit 1
    docker build -t "$TAGS" "$REPO_DIR"
fi

# 2. Swap containers
echo "Swapping containers..."

# Clean up any existing -old container
docker stop deductible-app-old 2>/dev/null || true
docker rm -f deductible-app-old 2>/dev/null || true

# Rename current to old and stop it
if docker ps -a --format '{{.Names}}' | grep -q "^deductible-app$"; then
    echo "Backing up current container to deductible-app-old"
    docker stop deductible-app || true
    docker rename deductible-app deductible-app-old || true
fi

# Run new container
echo "Starting new container: $TAGS"
if ! docker run -d \
    --name deductible-app \
    --restart always \
    --network=host \
    --env-file "$APP_ENV" \
    --cap-drop=ALL \
    --security-opt no-new-privileges \
    "$TAGS"; then
    echo "Error: Failed to start new container"
    # Rollback
    if docker ps -a --format '{{.Names}}' | grep -q "^deductible-app-old$"; then
        echo "Rolling back to deductible-app-old..."
        docker start deductible-app-old || true
        docker rename deductible-app-old deductible-app || true
    fi
    exit 1
fi

# Verify deployment
sleep 5
# Exact match check for container name and verify it's running
if docker ps --filter "name=^/deductible-app$" --filter "status=running" --format '{{.Names}}' | grep -q "^deductible-app$"; then
    echo "Successfully deployed $TAGS"
    echo "$TAGS" > "$STATE_FILE"
    docker rm -f deductible-app-old 2>/dev/null || true

    # Setup Caddy if domain is specified
    if [ -f "$APP_ENV" ]; then
        SITE_DOMAIN=$(grep '^SITE_DOMAIN=' "$APP_ENV" | cut -d'=' -f2- || true)
        if [ -n "$SITE_DOMAIN" ]; then
            echo "Setting up Caddy for $SITE_DOMAIN"
            docker rm -f caddy 2>/dev/null || true
            printf '%s\n' "${SITE_DOMAIN} {" '  reverse_proxy localhost:8080' '}' > /home/opc/Caddyfile
            docker run -d --name caddy --restart unless-stopped --network host \
                -v /home/opc/Caddyfile:/etc/caddy/Caddyfile:z \
                -v caddy_data:/data -v caddy_config:/config docker.io/library/caddy:2
        fi
    fi
else
    echo "New container failed to start or died shortly after. Rolling back..."
    docker stop deductible-app 2>/dev/null || true
    docker rm -f deductible-app 2>/dev/null || true
    if docker ps -a --format '{{.Names}}' | grep -q "^deductible-app-old$"; then
        docker start deductible-app-old || true
        docker rename deductible-app-old deductible-app || true
    fi
    exit 1
fi
