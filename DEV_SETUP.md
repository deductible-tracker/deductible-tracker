# Developer Setup — Deductible Tracker

This file documents the minimum environment and commands to run the project locally for development.

## Quick start (development using local Oracle Free)

1. Ensure you have Rust toolchain installed (recommended stable or nightly used by the project).
2. Copy `.env.example` to `.env` and adjust any values you need.
3. Start the app:

```bash
colima start --vm-type=vz --mount-type=virtiofs --cpu 2 --memory 3
cp .env.example .env
docker compose up --build
```

This starts the local Oracle Free container, runs the migration job once, and then starts the web server on port `8080`.

If the logs stop after Oracle prints `DATABASE IS READY TO USE!`, the next gate is the container healthcheck. The checked-in Compose file now probes the PDB from inside the container, so `migrate` and `app` should continue automatically once `FREEPDB1` is reachable.

Use `docker compose build` whenever you want to regenerate Tailwind output and the fingerprinted frontend assets baked into the image.

### Faster frontend iteration with a dev override

If you are changing files under `static/` and want to avoid a full image rebuild on every edit, use the checked-in dev override:

```bash
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up --build
```

What it changes:

- The `app` service runs from the Dockerfile `builder` stage, so Node.js, npm, and `node_modules` are available inside the container.
- `./static` is bind-mounted into `/app/static`, so changes to `index.html`, stylesheets, and frontend JavaScript are available to the container immediately.
- On app startup, frontend assets can be rebuilt inside the container instead of only during image build.

Useful toggles for local development:

- `DEV_SKIP_TAILWIND_BUILD=false` (default): rebuild Tailwind at app startup. Set to `true` if you are not editing Tailwind input CSS and want faster restarts.
- `DEV_SKIP_ASSET_REBUILD=false` (default): rebuild fingerprinted JS/CSS assets at app startup. Set to `true` if you are only changing HTML and want to keep the last generated asset manifest.

Typical frontend workflow:

```bash
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up --build -d
docker-compose restart app
```

That restart is enough to pick up most `static/` changes without rebuilding the image again.

By default development reads `DEV_ORACLE_USER`, `DEV_ORACLE_PASSWORD`, and `DEV_ORACLE_CONNECT_STRING`. It also falls back to `ORACLE_PDB_USER`, `ORACLE_PWD`, and `ORACLE_PDB_CONNECT_STRING`, which lets the same `.env` work for both `docker compose up` and host-side `cargo run` commands.

## Required environment variables (overview)

The app uses Oracle in both development and production. For local development the minimum variables are:

- `JWT_SECRET` — a string used to sign JWTs. Set to any random secret for local development.
- `RUST_ENV` — set to `development` for local runs (use `production` in production).
- `ALLOW_DEV_LOGIN` — set to `true` to enable `/auth/dev/login` for quick local sign-in.
- `DEV_USERNAME` / `DEV_PASSWORD` — credentials accepted by the dev login endpoint.

The server currently expects object storage configuration on startup. For local development you can either:

- Run a local S3-compatible service (MinIO) and set the object-storage env vars below to point at it, or
- Set the object storage vars to valid values for your remote object store (OCI/AWS), or
- Use placeholder values and skip functionality that requires uploads.

Object storage variables (required by the server):

- `OBJECT_STORAGE_ENDPOINT` — S3/OCI endpoint URL (e.g. `http://localhost:9000`).
- `OBJECT_STORAGE_BUCKET` — bucket name used to store receipts.
- `OCI_REGION` — region string (used by OpenDAL S3 configuration).
- `OCI_ACCESS_KEY_ID` — access key (or AWS access key) for object storage.
- `OCI_SECRET_ACCESS_KEY` — secret access key for object storage.

Database variables (development / local Oracle Free):

- `DEV_ORACLE_USER`, `DEV_ORACLE_PASSWORD`, `DEV_ORACLE_CONNECT_STRING` — used when `RUST_ENV=development`.

Database variables (production / Oracle ATP):

- `DB_USER`, `DB_PASSWORD`, `DB_CONNECT_STRING` — used when `RUST_ENV=production` to connect to Oracle.

Other useful variables:

- `ALLOWED_ORIGINS` — comma-separated origins for CORS (required in production).
- `RATE_LIMIT_PER_SECOND` / `RATE_LIMIT_BURST` — global request throttling controls.
- `AUTH_RATE_LIMIT_PER_SECOND` / `AUTH_RATE_LIMIT_BURST` — stricter throttling for `/auth/*` routes.
- `RUST_LOG` — logging configuration string (e.g. `info`).
- `PROPUBLICA_API_BASE_URL` — optional override for ProPublica endpoint base (defaults to `https://projects.propublica.org/nonprofits/api/v2`).

## Dev login

If `ALLOW_DEV_LOGIN=true` and the server runs with `RUST_ENV=development`, you can POST to `/auth/dev/login` with JSON `{ "username": "<user>", "password": "<pass>" }` to set a dev session cookie.

## Running with Docker Compose

`docker-compose.yml` is the default local development stack. Make sure to populate a `.env` file before running `docker compose up --build`.

### Docker Compose & local Oracle credentials

- Populate the following variables in your local `.env` (examples in `.env.example`): `ORACLE_PDB`, `ORACLE_PWD`, `ORACLE_CHARACTERSET`, `ENABLE_ARCHIVELOG`, `ENABLE_FORCE_LOGGING`, `ORACLE_PDB_USER`, and `ORACLE_PDB_CONNECT_STRING`.
- The Compose app and migration services automatically use the internal hostname `oracle-dev` to reach the database, so you do not need a separate Compose-specific connect string.
- Do NOT commit your `.env` file; keep secrets out of version control. Use `.env.example` as the committed reference with placeholder values.

## Notes & troubleshooting

- If you see errors about missing `OBJECT_STORAGE_*` envs when running locally, either set them to point at a local MinIO deployment or export placeholder values until you implement a local storage shim.
- Development uses the checked-in Oracle schema in `migrations/init.sql`; the default Compose stack runs the migration service automatically before the app starts.
- To run a type-check / build quickly, use `cargo check`.
- To inspect the readiness gate directly, run `docker-compose ps` and `docker-compose logs -f oracle-dev`. `app` waits for `oracle-dev` to become healthy, not just for the database process to print its startup banner.
- The dev override is intended for local-only frontend work. The default `docker-compose.yml` remains the production-like path that serves prebuilt assets from the image.

### OCR (ocrs + Mistral) setup (optional)

The project supports OCR for receipt processing using the `ocrs` crate for text extraction and Mistral AI for parsing the raw text into structured data.

To enable OCR functionality:

1. **Environmental Variables**: Set the following in your `.env` file:
   - `MISTRAL_API_KEY`: Your Mistral AI API key (required for structured extraction).
   - `MISTRAL_API_ENDPOINT`: Mistral API base URL (defaults to `https://api.mistral.ai/v1/conversations`).
   - `MISTRAL_MODEL`: Mistral model to use (defaults to `mistral-small-latest`).
   - `OCRS_MODEL_DIR`: Directory to cache `ocrs` ONNX/RTEN models (defaults to `/tmp/deductible-tracker-ocrs-models`).

2. **Build with the OCR feature**:
   ```bash
   RUST_ENV=development cargo build --features ocr
   ```

3. **Run the server**:
   ```bash
   RUST_ENV=development cargo run --features ocr
   ```

Notes:
- If the `ocr` feature is disabled or `MISTRAL_API_KEY` is missing, the server will still run, but OCR endpoints will return a warning or error.
- The `ocrs` engine will automatically download required models to `OCRS_MODEL_DIR` on the first run.

## Production OAuth & secret management

- Use a secrets manager (HashiCorp Vault, OCI Vault, AWS Secrets Manager, or GitHub Actions secrets) to store sensitive values: `JWT_SECRET`, OAuth client secrets, and object storage credentials.
- For quick local setup, copy `.env.example` to `.env` and fill values. Do NOT commit `.env` to VCS.
- Recommended: set `OAUTH_PROVIDERS` to the providers you will use (e.g. `GOOGLE`) and populate the provider-specific env vars (e.g. `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`).
- In production, inject secrets into the runtime environment (systemd unit, container environment, or cloud instance metadata) — do not bake secrets into images.

## Example: set secrets in shell for testing

```bash
export JWT_SECRET=$(openssl rand -hex 32)
export OAUTH_PROVIDERS=GOOGLE
export GOOGLE_CLIENT_ID=...
export GOOGLE_CLIENT_SECRET=...
```
