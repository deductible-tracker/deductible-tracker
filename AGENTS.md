# AGENTS.md

## Overview

- Deductible Tracker is a Rust and Axum backend with an Oracle database plus a vanilla JavaScript, Dexie, and Tailwind frontend.
- This file is the repository's primary, agent-agnostic instruction entry point.
- Reusable workflows live under `.agents/skills/` as plain Markdown `SKILL.md` files so they can be used by any coding agent.
- If a tool supports explicit rule or skill imports, point it at this file and the `.agents/skills/` directory rather than maintaining tool-specific copies.

## Start Here

- Read `docs/design-patterns/rust/README.md` before suggesting or changing Rust code.
- Review `src/db/core.rs` for the public DB surface.
- Review `static/js/db.js` for IndexedDB schema decisions.
- Review `static/js/sync.js` for offline sync behavior.
- For non-trivial work, start with `.agents/skills/using-agent-skills/SKILL.md` and then load only the skills that match the task.

## Skills and References

### Repo-specific skills

- `.agents/skills/frontend-feature/SKILL.md`
- `.agents/skills/rust-db-operation/SKILL.md`
- `.agents/skills/testing-patterns/SKILL.md`

### Vendored general-purpose skills

- The broader lifecycle skills in `.agents/skills/` are vendored from `addyosmani/agent-skills`.
- Specialist review personas live in `.agents/agents/`.
- Supporting checklists live in `.agents/references/`.
- Keep skill loading selective. Use the repo-specific skill first when both a local and generic skill apply.

## Development Commands

- Quick Rust check: `RUST_ENV=development cargo check`
- Quick run: `cargo run`
- Full stack: `docker-compose up --build`
- Frontend iteration: `docker-compose -f docker-compose.yml -f docker-compose.dev.yml up`
- Full test suite: `docker-compose -f docker-compose.test.yml up --build --abort-on-container-exit --exit-code-from test`
- Quick tests: `cargo test` and `npm run test:js`
- JS auto-fix and format: `npm run lint:js -- --fix` and `npm run format`

## Engineering Standards

- Consult `docs/design-patterns/rust/` before making Rust changes.
- Any Rust change must be followed by `cargo check` and `cargo clippy --fix --allow-dirty`. Fix all errors and warnings before finishing.
- Prefer `anyhow::Result` and `tracing` in Rust code.
- Use the async `deadpool-oracle` and `oracle-rs` APIs directly, positional placeholders (`:1`, `:2`), direct column projection, and shared row helpers.
- Reuse a single checked-out DB connection per logical unit of work unless there is a clear reason not to.
- All tests belong in `tests/` or `tests/js/`. Do not place tests in `src/` or `static/js/`.
- For DB mutations, use `log_revision` and capture `old_values` and `new_values`.
- State-changing endpoints need CSRF protection. Production cookies must be `Secure` and use `SameSite=Strict` or `SameSite=Lax` as appropriate.
- IndexedDB via Dexie is the UI source of truth. Use `Sync.queueAction` and `Sync.pushChanges` for sync flows.

## Documentation Rules

- Keep repository guidance agent-agnostic. Avoid naming a specific assistant unless a file exists only for that tool.
- Do not reintroduce git submodules for agent skills. Keep them vendored as plain files in the repo.
- Treat `AGENTS.md` as the canonical repo-wide agent guidance unless a closer nested `AGENTS.md` overrides it.