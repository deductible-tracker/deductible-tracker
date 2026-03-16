# Deductible Tracker

A production-grade charitable donation tracker and valuation engine, designed as a modern replacement for legacy tools like TurboTax's ItsDeductible. Built with a high-performance Rust backend and an offline-first PWA frontend.

## Key Features

- **Offline-First Architecture**: Fully functional without an active internet connection using IndexedDB (via Dexie.js) and Service Workers for background synchronization.
- **Automated Valuation Engine**: Integrated database with IRS-compliant Fair Market Value (FMV) estimates, seeded directly into the persistence layer.
- **Charity Intelligence**: Integrated with the ProPublica Nonprofits API for real-time charity verification and EIN lookups.
- **Intelligent Receipt Processing**: 
  - Secure storage in OCI Object Storage using S3-compatible APIs and presigned URLs.
  - Optional OCR (Optical Character Recognition) using Tesseract and Leptonica to extract dates and amounts from uploaded receipts.
- **Tax Optimization**: Real-time tax benefit estimates based on user-provided filing status, AGI, and marginal tax rates.
- **Privacy & Security**: JWT-based authentication with support for OAuth2 providers (Google) and secure session management via HttpOnly cookies.

## Technical Stack

### Backend (Rust)
- **Framework**: [Axum](https://github.com/tokio-rs/axum) on [Tokio](https://tokio.rs/) for asynchronous I/O.
- **Database**: [Oracle Database 23ai](https://www.oracle.com/database/23ai/) (Autonomous Database in production, Free Edition for local development).
- **OCR**: [Tesseract](https://github.com/tesseract-ocr/tesseract) via the `leptess` crate (gated by the `ocr` feature).
- **Authentication**: OAuth2 and JWT (using `jsonwebtoken` and `oauth2` crates).
- **Storage**: OCI Object Storage (S3-Compatible API).

### Frontend (Modern Web)
- **Engine**: Vanilla JavaScript (ES Modules) for a lightweight, dependency-minimal runtime.
- **Styling**: [Tailwind CSS 4.0](https://tailwindcss.com/) with native CSS nesting and modern browser primitives.
- **Persistence**: [Dexie.js](https://dexie.org/) for robust IndexedDB management.
- **PWA**: Service Workers for caching and background sync.

### Infrastructure & DevOps
- **Deployment**: Containerized deployment to OCI Ampere (ARM64) instances.
- **CI/CD**: GitHub Actions for automated testing, Docker builds, and SSH-based deployment.
- **Infrastructure as Code**: Terraform for managing OCI resources (VCN, ATP, Object Storage).

## Project Structure

The codebase is organized into modular "sections" to maintain clarity as the project grows:

- `src/main_sections/`: Core server logic (bootstrap, HTTP pipeline, asset management).
- `src/auth_sections/`: Authentication workflows (OAuth flows, profile management, JWT support).
- `src/db/core_sections/`: Domain-specific database operations (donations, charities, valuations).
- `src/db/oracle/`: Low-level Oracle OCI bindings and persistence logic.
- `src/routes/`: API endpoint definitions and request handlers.
- `src/ocr/`: Optional Tesseract-based OCR implementation.
- `static/`: Frontend source files (HTML, JS, CSS).
- `public/`: Generated and fingerprinted production assets.

## Development Setup

### Prerequisites

- **Rust**: 1.75+
- **Node.js**: 24.x (LTS recommended)
- **Oracle Instant Client**: Required for linking `rust-oracle`.
- **Docker**: For running the local Oracle Database Free container.

### Local Environment

1. **Initialize Environment**:
   ```bash
   cp .env.example .env
   ```
   Configure your Oracle and Object Storage credentials in `.env`.

2. **Start the Development Stack**:
   The project uses Docker Compose to orchestrate the app and an Oracle Database Free instance.
   ```bash
   docker-compose up --build
   ```
   This will run migrations, build Tailwind assets, and start the application on `http://localhost:8080`.

3. **Frontend Iteration**:
   For faster frontend development, use the dev override which bind-mounts the `static/` directory:
   ```bash
   docker-compose -f docker-compose.yml -f docker-compose.dev.yml up
   ```

4. **Running Tests**:
   ```bash
   cargo test
   npm run test:js
   ```

## Deployment

### Infrastructure
Provision OCI resources using Terraform:
```bash
cd terraform
terraform init
terraform apply
```

### CI/CD
Pushing to the `main` branch triggers the GitHub Actions workflow, which:
1. Builds a hardened Docker image (Oracle Linux 9 slim base).
2. Pushes the image to GitHub Container Registry (GHCR).
3. Deploys the image to the target OCI VM and executes database migrations.
