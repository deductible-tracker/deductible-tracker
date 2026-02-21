# Developer Setup — Deductible Tracker

This file documents the minimum environment and commands to run the project locally for development.

## Quick start (development using SQLite)

1. Ensure you have Rust toolchain installed (recommended stable or nightly used by the project).
2. Create a `.env` file or export environment variables shown below.
3. Start the app:

```bash
# optional: run migrations binary if you want to apply SQL migrations explicitly
cargo run --bin migrate

# Start the web server (development uses SQLite by default)
RUST_ENV=development cargo run
```

By default the code will create `dev.db` in the repo root unless you set `DEV_SQLITE_PATH`.

## Required environment variables (overview)

The app supports Oracle in production, but for local development the minimum variables are:

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

Database variables (production / Oracle):

- `DB_USER`, `DB_PASSWORD`, `DB_CONNECT_STRING` — used when `RUST_ENV=production` to connect to Oracle.

Other useful variables:

- `ALLOWED_ORIGINS` — comma-separated origins for CORS (required in production).
- `RUST_LOG` — logging configuration string (e.g. `info`).
- `PROPUBLICA_API_BASE_URL` — optional override for ProPublica endpoint base (defaults to `https://projects.propublica.org/nonprofits/api/v2`).

## Dev login

If `ALLOW_DEV_LOGIN=true` and the server runs with `RUST_ENV=development`, you can POST to `/auth/dev/login` with JSON `{ "username": "<user>", "password": "<pass>" }` to set a dev session cookie.

## Running with Docker Compose

`docker-compose.yml` in the repo provides services for the app and migrations. Make sure to populate a `.env` file with the required variables before running `docker-compose up --build`.

## Notes & troubleshooting

- If you see errors about missing `OBJECT_STORAGE_*` envs when running locally, either set them to point at a local MinIO deployment or export placeholder values until you implement a local storage shim.
- The development code creates necessary SQLite tables at startup (`init_pool`), so explicit migration steps are optional for local work.
- To run a type-check / build quickly, use `cargo check`.

### OCR (Tesseract) setup (optional)

The project supports local OCR using Tesseract/Leptonica via the `leptess` crate. This is optional — builds will succeed without Tesseract, but OCR functionality will return a clear error until enabled.

To enable real local OCR:

1. Install system dependencies (macOS example):

```bash
brew install tesseract
brew install leptonica
```

2. Build the project with the `ocr` feature enabled:

```bash
RUST_ENV=development cargo build --features ocr
```

3. Run the server (with OCR feature enabled):

```bash
RUST_ENV=development cargo run --features ocr
```

Notes:
- If you don't enable the `ocr` feature or you don't have Tesseract installed, the server will still build and run; calling the OCR endpoint will return an error indicating OCR is not enabled.
- On Linux or other platforms, use your package manager to install `tesseract` and `leptonica`.

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

If you want, I can add a small `scripts/` helper to validate required env vars at startup and produce a helpful error message listing missing keys.

If you'd like, I can create a `.env.example` file with recommended local values and a small `docker-compose` snippet for MinIO.
