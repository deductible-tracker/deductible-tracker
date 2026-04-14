---
name: testing-patterns
description: Use when adding or updating Rust or JavaScript tests in deductible-tracker, including integration, Jest, and verification workflow.
---

# Skill: Testing Patterns (JS & Rust)

This skill provides the standard procedures for verifying changes in the `deductible-tracker` codebase.

## 1. Full Suite Testing (Rust & JS)

**To run the entire test suite (including Rust and JS tests), use Docker Compose:**

- **Command**: `docker-compose -f docker-compose.test.yml up --build --abort-on-container-exit --exit-code-from test`
- This command ensures a clean environment with a healthy Oracle container, runs migrations, and executes both backend and frontend tests.

## 2. Rust Backend Testing

**All tests MUST be placed in the `tests/` directory.**

- **Integration/Unit**: Use `.rs` files in `tests/`.
- **Manual Command**: For quick checks, use `RUST_ENV=development cargo test`.
- **Warning Check**: ALWAYS run `cargo check` and fix all warnings before final submission.
- **Internal Access**: To test private/internal functions, use the `include!` pattern.
  ```rust
  mod my_internal_test {
      include!("../src/path/to/my/module.rs");
      #[test]
      fn my_private_test() { ... }
  }
  ```

## 2. JavaScript Frontend Testing

**All tests MUST be placed in `tests/js/`.**

- **NEVER** place tests in `static/js/`.
- **Command**: Run `npm run test:js`.
- **Mocking**: Use Jest mocks for `api-client.js` or `db.js` (Dexie).
- **Coverage**: Focus on services in `static/js/services/`.

## 3. General Principles

- **Reproduction**: For any bug fix, create a test case that fails before the fix and passes after.
- **Surgical Changes**: Tests should be focused and avoid testing unrelated system components.
- **Wait for Readiness**: When suggesting tests, include any necessary logic to wait for background tasks (like sync or OCR) to complete.
- **Audit Logs**: For DB writes, verify that `RevisionLogEntry` entries are created in the `audit_revisions` table.
