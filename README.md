# Deductible Tracker

A production-grade charitable donation tracker and valuation engine, designed as a modern replacement for legacy tools like TurboTax's ItsDeductible. Built with a React 19 SPA frontend, Hono API backend, and Drizzle ORM communicating with a Cloudflare D1 SQLite database.

## Key Features

- **Offline-First Architecture**: Fully functional without an active internet connection using IndexedDB (via Dexie.js) for background synchronization.
- **Automated Valuation Engine**: Integrated database with IRS-compliant Fair Market Value (FMV) estimates, seeded directly into the persistence layer.
- **Charity Intelligence**: Integrated with the ProPublica Nonprofits API for real-time charity verification and EIN lookups.
- **Intelligent Receipt Processing**:
  - Secure storage in OCI Object Storage using S3-compatible APIs and presigned URLs.
  - Mistral OCR API integration for high-accuracy text extraction and structured donation data pre-fill (supporting PDF, Word, Images, and more).
- **Tax Optimization**: Real-time tax benefit estimates based on user-provided filing status, AGI, and marginal tax rates.
- **Privacy & Security**: WebAuthn/Passkey-based zero-knowledge vault client-side encryption. Encrypts sensitive donation amounts, notes, and charity names in-browser using AES-GCM before syncing to the cloud.

## Technical Stack

### Backend

- **API Framework**: [Hono](https://hono.dev/) running on Cloudflare Workers/Pages.
- **Database & ORM**: [Drizzle ORM](https://orm.drizzle.team/) communicating with [Cloudflare D1](https://developers.cloudflare.com/d1/) (SQLite-compatible) persistence layer.
- **OCR**: Integrated [Mistral OCR API](https://docs.mistral.ai/capabilities/document_ai/basic_ocr/) for processing receipts into structured JSON data.
- **Authentication**: JWT-based session cookies with support for OAuth2 (Google) and Developer logins.

### Frontend

- **SPA Engine**: React 19 + React Router 8 bundled with Vite.
- **Styling**: [Tailwind CSS 4.0](https://tailwindcss.com/) with native CSS nesting and modern browser primitives.
- **Persistence**: [Dexie.js](https://dexie.org/) for robust IndexedDB management.

## Project Structure

- `AGENTS.md`: Repository-wide instructions for coding agents.
- `drizzle.config.ts`: Configuration for Drizzle migrations schema mapping.
- `index.html`: Main HTML entry page.
- `vite.config.ts`: Configuration for React bundling and Hono API devServer.
- `wrangler.json` / `wrangler.toml`: Cloudflare Worker and D1 Database binding configs.
- `src/db/`: Database schema definitions (`schema.ts`) and initialization entry.
- `src/services/`: Client-side database helper, crypto vault signers, API client wrappers, and offline sync engines.
- `src/routes/`: Router layout layouts and application routes (Dashboard, Donations, Charities, Reports, Personal).

## Development Setup

See [DEV_SETUP.md](DEV_SETUP.md) for quick-start local environment configurations.
