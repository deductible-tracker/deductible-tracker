# Developer Setup — Deductible Tracker

This file documents the minimum environment and commands to run the project locally for development.

## Quick Start (Development)

1. Ensure you have **Node.js** (v20+ recommended) and npm installed.
2. Copy `.env.example` to `.env` and adjust any values you need.
3. Install dependencies and start the app:

```bash
cp .env.example .env
npm install --legacy-peer-deps
npm run dev
```

This starts the Vite development server (which also mounts the Hono backend API dev server) on port `8080`.
The database tables and initial test accounts seed automatically on first boot.

## Development Commands

- **`npm run dev`**: Starts the local development server at `http://localhost:8080`.
- **`npm run build`**: Compiles and bundles the React frontend and builds production assets into `dist/`.
- **`npm run db:generate`**: Generates SQL migration files from your schema (`src/db/schema.ts`) using `drizzle-kit`.
- **`npx wrangler deploy`**: Builds and deploys the app to Cloudflare Pages/Workers.

## Required Environment Variables (Overview)

Copy `.env.example` to `.env` and fill in values for local testing:

- **`JWT_SECRET`**: A secret string used to sign session JWTs.
- **`ALLOW_DEV_LOGIN`**: Set to `true` to enable `/auth/dev/login` for quick local sign-in.
- **`DEV_USERNAME` / `DEV_PASSWORD`**: Credentials accepted by the dev login endpoint (e.g. `admin` / `dev-password-for-demo`).

### OCR (Mistral OCR API) Setup
The project uses the [Mistral OCR API](https://docs.mistral.ai/capabilities/document_ai/basic_ocr/) for high-accuracy receipt processing.
To enable OCR functionality, set:
- `MISTRAL_API_KEY`: Your Mistral AI API key (required).
- `MISTRAL_API_ENDPOINT`: Endpoint URL (defaults to `https://api.mistral.ai/v1/ocr`).
- `MISTRAL_MODEL`: Model name (defaults to `mistral-ocr-latest`).

### OCI Object Storage Setup
To upload and process receipt attachments, configure OCI/S3-compatible Object Storage credentials:
- `OBJECT_STORAGE_ENDPOINT`: S3/OCI endpoint URL.
- `OBJECT_STORAGE_BUCKET`: Bucket name used to store receipt files.
- `OCI_REGION`: Region identifier (e.g. `us-ashburn-1`).
- `OCI_ACCESS_KEY_ID`: Access key ID.
- `OCI_SECRET_ACCESS_KEY`: Secret access key.

### Google OAuth Configuration
To allow users to login via Google OAuth:
- `GOOGLE_CLIENT_ID`: Your Google OAuth Client ID.
- `GOOGLE_CLIENT_SECRET`: Your Google OAuth Client Secret.
