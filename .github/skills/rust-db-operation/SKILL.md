---
name: rust-db-operation
description: Guidance for adding new database operations in the Rust backend using oracle-rs and deadpool-oracle, including modeling, implementation, query design, and audit logging.
---

# Skill: Adding a new Rust DB Operation

This skill provides a step-by-step workflow for implementing new persistence logic in the `deductible-tracker` backend.

## 1. Define the Models

Add input and output structs to `src/db/models.rs`.

- Use `New...` for creations.
- Use `...Patch` for updates (with `Option` fields).
- Ensure `derive(Debug, Clone)` for inputs and `Serialize, Deserialize, Debug, Clone` for outputs.
- Use `chrono::NaiveDate` for dates and `DateTime<Utc>` for timestamps.

## 2. Implement in the Current DB Layers

Use the repo's current split between app-facing wrappers and Oracle-specific SQL:

- Add orchestration or cross-domain glue in `src/db/core_sections/` when the operation spans multiple domains.
- Add Oracle SQL and row-mapping logic in `src/db/oracle/` when the operation is backend-specific.
- Expose the operation through the matching public wrapper in `src/db/*.rs` or `src/db/core_sections/bootstrap/runtime_and_bootstrap.rs`.

- **Async DB access**: The codebase uses `deadpool-oracle` plus `oracle-rs`. Prefer `let conn = pool.get().await?;` and call `conn.query(...).await?`, `conn.execute(...).await?`, or `conn.query_single(...).await?` directly.
- **Parameters**: Use positional placeholders `:1`, `:2` and pass binds through `crate::oracle_params![...]` where practical.
- **Row decoding**: Prefer direct column selection plus the shared helpers in `src/db/oracle/mod.rs` such as `row_string`, `row_opt_string`, `row_i64`, `row_f64`, `row_bool`, `row_naive_date`, and `row_datetime_utc`.
- **Avoid packed payloads**: Do not combine multiple fields into delimiter-packed strings or JSON payloads unless a verified `oracle-rs` limitation makes that necessary.
- **Projection discipline**: Select only the columns needed by the caller. Avoid `SELECT *`, unnecessary casts, and repeated lookups inside loops.
- **Connection usage**: Reuse a single checked-out connection for a logical unit of work. Do not fetch multiple pool connections inside one request path unless there is a clear reason.
- **Audit Logging**: For any mutation (create/update/delete), you MUST create a `RevisionLogEntry` and call `log_revision`.
  - Capture `old_values` and `new_values` as JSON strings.
  - Use `Uuid::new_v4().to_string()` for the revision ID.
- **Transactions**: Explicitly `commit()` for writes. Keep transactions short because the production target is a small app server talking to an Always Free Autonomous Database with tight session limits.
- **Autonomous DB performance**: Design for low session count and low round-trip overhead. Prefer indexed point lookups, batched work where possible, and query shapes that keep CPU and network usage modest.

## 3. Register in Core

If adding a new file in `core_sections`, `include!` it in `src/db/core.rs`.

## 4. Add the Public Wrapper

Expose the function in the corresponding domain module (for example `src/db/donations.rs`) or in `src/db/core_sections/bootstrap/runtime_and_bootstrap.rs` when it is part of the high-level DB surface.

## 5. Validation

- Run `cargo check` to ensure types and imports are correct.
- Run `cargo clippy --fix --allow-dirty` after Rust changes and resolve remaining warnings.
- **Add a test in `tests/`** (e.g., `tests/integration_donations.rs`).
- **STRICT**: Never place tests in `src/`.
- Prefer a focused integration test that exercises the real Oracle path for bug fixes or query changes.
- Use the `include!` trick in the test file if you need to test internal helpers.

## Example Pattern

```rust
pub async fn my_operation(pool: &Pool, input: &MyInput) -> anyhow::Result<()> {
  let conn = pool.get().await?;
  conn.execute(
    "INSERT INTO table (id, name) VALUES (:1, :2)",
    &crate::oracle_params![input.id.clone(), input.name.clone()],
  )
  .await?;
  conn.commit().await?;

    // Log revision here...
    Ok(())
}
```
