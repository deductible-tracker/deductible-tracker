# 2.1 Use borrowed types for arguments

Description

Prefer borrowed types (`&str`, `&[T]`, `&T`) over owning references like `&String` or `&Vec<T>`. This increases API flexibility and avoids extra indirection.

Example

```rust
fn three_vowels(word: &str) -> bool { /* inspect chars */ }
```

Advantages

- Accepts both owned and borrowed inputs without allocation.

Disadvantages

- None significant; prefer clarity when ownership is required.

See also

- `AsRef`, `Borrow`
