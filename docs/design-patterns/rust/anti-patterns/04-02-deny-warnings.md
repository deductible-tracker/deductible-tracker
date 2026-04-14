# #![deny(warnings)]

Description

Annotating a crate root with `#![deny(warnings)]` will fail the build on any compiler warning. While
the intention is to keep code clean, this can make crates brittle across compiler/lint changes.

Alternatives

- Set `RUSTFLAGS="-D warnings"` in CI rather than forcing it in code.
- Deny specific lints instead of all warnings.

Last change: 2026-01-03, commit:f279f35
