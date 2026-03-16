# 2.12 `#[non_exhaustive]` and private fields for extensibility

Description

Use `#[non_exhaustive]` on public structs/enums to allow future extension without breaking downstream crates. For intra-crate extensibility, use a private trailing field to force `..` patterns.

Advantages

- Enables non-breaking additions across crate boundaries.

Disadvantages

- Reduces ergonomics for consumers; forces wildcard patterns for enums.
