---
name: rust-db-operation
description: Guidance for adding new database operations in the Rust/Oracle backend, including modeling, implementation, and audit logging.
---

# Skill: Adding a new Rust DB Operation

This skill provides a step-by-step workflow for implementing new persistence logic in the `deductible-tracker` backend.

## 1. Define the Models
Add input and output structs to `src/db/models.rs`.
- Use `New...` for creations.
- Use `...Patch` for updates (with `Option` fields).
- Ensure `derive(Debug, Clone)` for inputs and `Serialize, Deserialize, Debug, Clone` for outputs.
- Use `chrono::NaiveDate` for dates and `DateTime<Utc>` for timestamps.

## 2. Implement in Core Sections
Add the logic in a new or existing file under `src/db/core_sections/`.
- **Sync to Async**: Wrap Oracle driver calls in `tokio::task::spawn_blocking`.
- **Cloning**: Clone the `DbPool` and any arguments before moving them into the `spawn_blocking` closure.
  ```rust
  let p = pool.clone();
  let arg_cloned = arg.clone();
  task::spawn_blocking(move || {
      let conn = p.get()?;
      // ... SQL logic
  })
  ```
- **SQL**: Use positional placeholders `:1`, `:2`.
- **Audit Logging**: For any mutation (create/update/delete), you MUST create a `RevisionLogEntry` and call `log_revision`.
  - Capture `old_values` and `new_values` as JSON strings.
  - Use `Uuid::new_v4().to_string()` for the revision ID.

## 3. Register in Core
If adding a new file in `core_sections`, `include!` it in `src/db/core.rs`.

## 4. Add the Public Wrapper
Expose the function in the corresponding domain module (e.g., `src/db/donations.rs`). This is usually a thin wrapper that delegates to `super`.

## 5. Validation
- Run `cargo check` to ensure types and imports are correct.
- **Add a test in `tests/`** (e.g., `tests/integration_donations.rs`).
- **STRICT**: Never place tests in `src/`.
- Use the `include!` trick in the test file if you need to test internal helpers.

## Example Pattern
```rust
pub async fn my_operation(pool: &DbPool, input: &MyInput) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    task::spawn_blocking(move || {
        let conn = p.get()?;
        conn.execute("INSERT INTO table (id) VALUES (:1)", &[&input.id])?;
        conn.commit()?;
        Ok(())
    }).await??;
    
    // Log revision here...
    Ok(())
}
```
