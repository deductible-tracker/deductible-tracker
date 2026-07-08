# AGENTS.md

## Overview

- Deductible Tracker is a Hono API and Vite React SPA frontend using a Cloudflare D1 SQLite database, Dexie, and Tailwind CSS.
- This file is the repository's primary, agent-agnostic instruction entry point.
- Reusable workflows live under `.agents/skills/` as plain Markdown `SKILL.md` files so they can be used by any coding agent.

## Start Here

- Review `src/db/schema.ts` for the public Drizzle SQLite schema.
- Review `src/services/db.ts` for IndexedDB client-side schema.
- Review `src/services/sync.ts` for offline sync behavior.
- For non-trivial work, start with `.agents/skills/using-agent-skills/SKILL.md` and then load only the skills that match the task.

## Development Commands

- Local iteration: `npm run dev` (starts Hono API + React devServer)
- Production build: `npm run build`
- DB schema migrations: `npm run db:generate`
- Deployment: `npx wrangler deploy`
- Quick tests: `npm run test:js`

## Engineering Standards

- Use Drizzle ORM to perform SQLite database queries.
- State-changing API endpoints must verify `csrf_token` matching the `X-CSRF-Token` header.
- Capturing changes: Use `logRevision` and log `oldValues` and `newValues` for audits.
- IndexedDB via Dexie is the client-side UI source of truth. Use `Sync.queueAction` and `Sync.pushChanges` for offline sync workflows.

## Documentation Rules

- Keep repository guidance agent-agnostic. Avoid naming a specific assistant.
- Fresh clones are self-contained and do not require submodule initialization.