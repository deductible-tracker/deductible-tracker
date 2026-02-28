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
- **Database (local dev)**: SQLite (`dev.db` by default)
- **Storage**: OCI Object Storage (S3-Compatible API)
- **Frontend**: Vanilla JS (ES Modules) + TailwindCSS + Dexie (IndexedDB)

## Frontend Asset Optimization

- Fingerprinted JavaScript and CSS assets under `static/assets/` are generated and minified automatically by the backend asset preparation pipeline at startup.
- `npm run tailwind:build` still performs Tailwind compilation with minification for `tailwind.css` before fingerprinting.

## Setup & Deployment

### Prerequisites

- Rust & Cargo
- Node.js + npm
- Oracle Cloud (OCI) CLI configured
- Docker (for production build)

### Local Development

1. **Environment Variables**:
   Create a `.env` file with:
   ```bash
   RUST_ENV=development
   SQLITE_DB_PATH=dev.db
   OCI_ACCESS_KEY_ID=...
   OCI_SECRET_ACCESS_KEY=...
   OBJECT_STORAGE_ENDPOINT=...
   OBJECT_STORAGE_BUCKET=...
   JWT_SECRET=...
   ```

   For production (`RUST_ENV=production`), set Oracle credentials:
   ```bash
   DB_USER=...
   DB_PASSWORD=...
   DB_CONNECT_STRING=...
   ```

2. **Start Local Server**:
   ```bash
   cargo run
   ```

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
- `src/db/oracle/` and `src/db/sqlite/`: backend-specific persistence.
- `static/`: frontend assets (PWA).

## Docker Notes

- `Dockerfile` builds both `deductible-tracker` and `migrate` binaries and ships
   them in an Oracle Linux 9 runtime image.
- `docker-compose.yml` expects a `.env` file with the production/runtime env vars.
- For Oracle wallet usage, mount `Wallet_deductibledb` and set `TNS_ADMIN` to the
   mounted path inside the container.
