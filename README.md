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
- **Database**: Oracle Autonomous Database (ATP)
- **Storage**: OCI Object Storage (S3-Compatible API)
- **Frontend**: Vanilla JS (ES Modules) + TailwindCSS

## Setup & Deployment

### Prerequisites

- Rust & Cargo
- Oracle Cloud (OCI) CLI configured
- Docker (for production build)

### Local Development

1. **Environment Variables**:
   Create a `.env` file with:
   ```bash
   DB_USER=...
   DB_PASSWORD=...
   DB_CONNECT_STRING=...
   OCI_ACCESS_KEY_ID=...
   OCI_SECRET_ACCESS_KEY=...
   OBJECT_STORAGE_ENDPOINT=...
   OBJECT_STORAGE_BUCKET=...
   JWT_SECRET=...
   ```

2. **Start Local Server**:
   ```bash
   cargo run
   ```

3. **Serve Frontend**:
   The backend serves the API on port 8080.
   ```bash
   cd static
   python3 -m http.server 3000
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
   Set `DYNAMODB_TABLE` and `S3_BUCKET` in your Lambda configuration.

## Project Structure

- `src/main.rs`: API Router & Entry point.
- `src/db/`: DynamoDB Models.
- `src/routes/`: Business logic.
- `static/`: Frontend assets (PWA).
