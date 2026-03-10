# 2.3 Constructors

Description

Rust does not have language-level constructors; use an associated `new()` function for conventional construction and implement/derive `Default` when appropriate.

Example

```rust
impl Second {
    pub fn new(value: u64) -> Self { Self { value } }
}
```

Advantages

- Clear, expected API for creating instances.

Disadvantages

- None; prefer also providing `Default` when zero-arg construction makes sense.
