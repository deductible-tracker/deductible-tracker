# 2.4 The `Default` Trait

Description

Implement or derive `Default` to provide a zero-argument constructor and enable `..Default::default()` partial initialization.

Example

```rust
#[derive(Default)]
struct MyConfiguration { output: Option<String>, search_path: Vec<String> }
```

Advantages

- Enables ergonomic partial initialization and usage with many std APIs.

Disadvantages

- Only one `Default` per type; choose sensible defaults.
