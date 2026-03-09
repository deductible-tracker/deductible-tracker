# Valuation DB Import Plan

Goal: provide the full valuation database (500+ items) to the client for fast lookups and autocomplete while keeping initial page load reasonable.

Options considered

- Client-seeded static file (recommended):
  - Precompute a compressed JSON (or NDJSON) of all valuation items and ship it under `static/` (e.g. `static/data/valuation.json.gz`).
  - On first run, or lazily on demand, client code fetches and imports entries into the Dexie `val_items` store.
  - Pros: simple to implement, fast client searches (IndexedDB), works offline after seed.
  - Cons: initial download size; compressing and gzipping mitigates this.

- Server-backed search API:
  - Implement `/api/val/search?q=<term>&limit=20` that performs server-side search (full-text or prefix) and returns results.
  - Pros: smaller client payloads, can index and optimize on server.
  - Cons: requires server storage, indexing, and additional infra; offline not available unless cached.

Recommendation

- Start with client-seeded static file approach for parity and offline support.

Implementation steps (client-seeded)

1. Obtain full valuation dataset (CSV/JSON). Clean and normalize fields: `id`, `category_id`, `name`, `description`, `value`.
2. Create a script (`tools/generate_valuation_json.js` or similar) that converts CSV -> compressed JSON in `static/data/valuation.json.gz`.
3. Add a small loader in `static/js/seed.js` that checks if `val_items` has entries; if not, fetch `/data/valuation.json.gz`, stream/parse, and bulk-insert into `db.val_items` using Dexie's bulk API.
4. Add progress UI (optional) and ensure seeding runs in a non-blocking task. For large imports, consider chunked imports (e.g., 500 items at a time) to avoid UI jank.
5. Add a fallback search endpoint on server if desired later.

Estimated effort: small → moderate (1–2 days) depending on dataset cleanup and UX polish.

Next steps I can take now:
- Add the `tools/generate_valuation_json.js` script and modify `static/js/seed.js` to support lazy import, or
- Implement a server-backed search endpoint for on-demand lookups.
