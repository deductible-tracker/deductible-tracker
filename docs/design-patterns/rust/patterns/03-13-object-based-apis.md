# Object-Based APIs (FFI)

Description

When designing APIs exposed to other languages, prefer object-based designs: keep encapsulated types
owned by Rust and opaque to foreign callers, and expose transactional types transparently. This reduces
the surface area of unsafety and clarifies ownership and lifetimes.

Discussion

POSIX DBM is a canonical example; prefer consolidating ownership and avoiding iterators that outlive
their owners in FFI interfaces.

Last change: 2026-01-03, commit:f279f35
