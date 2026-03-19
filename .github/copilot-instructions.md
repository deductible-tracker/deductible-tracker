# GitHub Copilot Instructions for deductible-tracker

## Purpose

This file orients GitHub Copilot to the repository's layout, common development workflows, and high-level architectural standards. For detailed workflows, refer to the specialized **Skills** in `.github/skills/`.

## Project Overview

- **Backend**: Rust (Axum, Oracle DB via `r2d2`).
- **Frontend**: Vanilla JS (ES6 modules), Dexie.js (IndexedDB), TailwindCSS v4.
- **Key Directories**:
  - `src/db/core_sections/`: Internal DB implementation (included via `include!`).
  - `static/js/services/`: Frontend business logic and API interaction.
  - `tests/`: ALL tests (Rust and JS) reside here.
  - `docs/design-patterns/rust/`: Mandatory Rust style guide.

## Development workflows (common commands)

- **Quick Check**: `RUST_ENV=development cargo check` (Always fix warnings!)
- **Quick Run**: `cargo run` (Requires local Oracle or `oracle-dev` container)
- **Full Run**: `docker-compose up --build` (Builds & runs full stack: Oracle + App)
- **JS/Asset Dev**: `docker-compose -f docker-compose.dev.yml up --build` (Mounts local `./static` for fast JS iteration)
- **Testing (Full Suite)**: `docker-compose -f docker-compose.test.yml up --build --abort-on-container-exit --exit-code-from test`
- **Format**: `cargo fmt` and `npm run format`

### Linting & Formatting (JS)

- **Auto-fix on change**: For any JavaScript/TypeScript/frontend changes or any files targeted by ESLint/Prettier, run automatic fixes before committing or opening a PR:
  - `npm run lint:js -- --fix` (runs ESLint with auto-fix)
  - `npm run format` (runs Prettier to reformat files)
  - Run `npm run format:check` in CI to catch remaining formatting issues.
  - **Fix all errors AND warnings** before final submission.
   - **Unused variables**: Prefer removing unused variables instead of commenting them out. Before removing, perform a repo-wide search to confirm the symbol isn't referenced elsewhere, then run `npm run lint:js -- --fix` and `npm run format` to ensure no new warnings are introduced.

## Core Engineering Standards (CRITICAL)

1. **Consult In-Repo Documentation**: ALWAYS check `docs/design-patterns/rust/` before suggesting Rust code.
2. **Rust Engineering**:
   - **Anytime Rust code is changed, ALWAYS run `cargo check` and `cargo clippy --fix --allow-dirty`.**
   - **Fix all errors AND warnings** before final submission.
   - Use `anyhow::Result` and `tracing`.
3. **Oracle DB Patterns**:
   - Wrap all DB operations in `tokio::task::spawn_blocking`.
   - Clone the `DbPool` and arguments before moving into the closure.
   - Use positional placeholders (`:1`, `:2`).
4. **Testing (STRICT)**:
   - **All tests MUST be placed in the `tests/` directory.**
   - NEVER place unit tests alongside source files.
5. **Security**:
   - Cookies: `SameSite=Strict/Lax`, `Secure` in production.
   - Audit Logging: Use `log_revision` for all DB mutations.
   - CSRF Protection: Include CSRF protection for state-changing endpoints. Use anti-forgery tokens (rotate per session) and validate server-side; `SameSite` cookies are a useful additional defense.
6. **Offline-First**:
   - IndexedDB (Dexie) is the source of truth for the UI.
   - Use `Sync.queueAction` and `Sync.pushChanges` for server synchronization.

## Specialized Skills

Refer to these for step-by-step implementation guidance:

- **Adding a DB operation**: `.github/skills/rust-db-operation/SKILL.md`
- **Adding a Frontend feature**: `.github/skills/frontend-feature/SKILL.md`
- **Adding a Test**: `.github/skills/testing-patterns/SKILL.md`

## Files to Consult First

- `docs/design-patterns/rust/README.md`
- `src/db/core.rs`
- `static/js/db.js`
- `static/js/sync.js`
