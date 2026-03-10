# Contain unsafety in small modules

Description

If you have unsafe code, create the smallest possible module that upholds the invariants and expose a
safe, ergonomic interface around it.

Advantages

- Restricts unsafe code needing audit
- Easier to reason about the outer safe API

Last change: 2026-01-03, commit:f279f35
