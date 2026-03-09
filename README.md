# Deductible Tracker

A production-grade serverless charitable donation tracker replacing TurboTax's ItsDeductible.

## Features

- **Offline-First**: Fully functional without internet (IndexedDB + Background Sync).
- **Valuation Database**: 500+ items with IRS-compliant FMV estimates.
- **Charity Search**: Integrated with ProPublica Nonprofits API.
- **Receipts**: S3 storage with presigned URLs.
- **Privacy**: Partner sharing and secure storage.

## Architecture

- **Backend**: Rust (Axum on OCI Ampere)
- **Database (prod)**: Oracle Autonomous Database (ATP)
- **Database (local dev)**: Oracle Database Free container
- **Storage**: OCI Object Storage (S3-Compatible API)
- **Frontend**: Vanilla JS (ES Modules) + TailwindCSS + Dexie (IndexedDB)

## Frontend Asset Optimization

- Fingerprinted JavaScript and CSS assets under `public/assets/` are generated and minified automatically during image builds.
- `npm run tailwind:build` still performs Tailwind compilation with minification for `tailwind.css` before fingerprinting.

## Setup & Deployment

### Prerequisites

- Rust & Cargo
- Node.js + npm
- Oracle Cloud (OCI) CLI configured
- Docker (for production build)

### Local Development

1. **Environment Variables**:
   Create a `.env` file from `.env.example` and adjust values as needed.

   Minimal local example:
   ```bash
   cp .env.example .env
   ```

   The default local stack reads `ORACLE_PDB_USER`, `ORACLE_PWD`, and `ORACLE_PDB_CONNECT_STRING` for development. Host-side Rust commands also accept `DEV_ORACLE_USER`, `DEV_ORACLE_PASSWORD`, and `DEV_ORACLE_CONNECT_STRING` if you want explicit dev-only names.

   For production (`RUST_ENV=production`), set Oracle credentials separately:
   ```bash
   DB_USER=...
   DB_PASSWORD=...
   DB_CONNECT_STRING=...
   ```

2. **Start Local Server**:
   ```bash
   colima start --vm-type=vz --mount-type=virtiofs --cpu 2 --memory 3
   docker-compose up --build
   ```

   This starts the local Oracle container, runs the migration service once, and then starts the app on port `8080`.

   Rebuild the image with `docker-compose build` whenever you want refreshed Tailwind output and fingerprinted frontend assets.

3. **Build Tailwind CSS**:
   ```bash
   npm install
   npm run tailwind:watch
   ```

4. **Serve Frontend**:
   The backend serves the API on port 8080.
   ```bash
   cd static
   python3 -m http.server 3000
   ```

For one-off production CSS generation:

```bash
npm run tailwind:build
```

### Production Deployment

1. **Infrastructure**:
   ```bash
   cd terraform
   terraform init
   terraform apply
   ```

2. **Build Docker Image**:
   ```bash
   make docker-build
   ```

3. **Environment Variables**:
    Set production runtime variables for Oracle + object storage, including:
    `RUST_ENV=production`, `DB_USER`, `DB_PASSWORD`, `DB_CONNECT_STRING`,
    `OBJECT_STORAGE_ENDPOINT`, `OBJECT_STORAGE_BUCKET`, `OCI_REGION`,
    `OCI_ACCESS_KEY_ID`, `OCI_SECRET_ACCESS_KEY`, `JWT_SECRET`, and `ALLOWED_ORIGINS`.

## Project Structure

- `src/main.rs`: server entry point and module wiring.
- `src/main_sections/`: HTTP/server organization by concern.
   - `bootstrap/`: startup, state, router bootstrap.
   - `http/`: middleware, handlers for index/fallback/cache policy.
   - `assets/`: fingerprint/minification helper pipeline.
- `src/auth.rs`: auth module root and include wiring.
- `src/auth_sections/`: auth organization by concern.
   - `flow/`: OAuth login/callback orchestration.
   - `profile/`: dev login/logout/me/profile update handlers.
   - `support/`: token/provider helpers and shared auth utilities.
- `src/db/core.rs`: DB facade root and include wiring.
- `src/db/core_sections/`: DB organization by domain.
   - `bootstrap/`: pool/runtime init and shared setup.
   - `donations/`: donation/receipt operations and valuations.
   - `charities/`: charity CRUD and receipt OCR metadata updates.
- `src/routes/`: API route handlers.
- `src/db/oracle/`: Oracle-backed persistence used in both development and production.
- `static/`: frontend assets (PWA).

## Docker Notes

- `Dockerfile` builds both `deductible-tracker` and `migrate` binaries and ships
   them in an Oracle Linux 9 runtime image.
- `docker-compose.yml` is now the default local development stack: Oracle Free,
   one-shot migrations, and the app.
- `docker compose build` rebuilds Tailwind output and fingerprinted frontend assets into the image.
- `docker compose up` starts the already-built image and does not regenerate frontend assets.
- Copy `.env.example` to `.env` before running `docker compose up --build`.
- The Compose app container overrides the Oracle connect string to use the
   internal hostname `oracle-dev`, so the same `.env` can still use
   `//localhost:1521/FREEPDB1` for host-side `cargo run` commands.
