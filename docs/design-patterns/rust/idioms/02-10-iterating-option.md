# 2.10 Iterating over an `Option`

Description

`Option<T>` implements `IntoIterator`; it can be used with iterator combinators and to extend collections. Use `std::iter::once` if always `Some`.

Example

```rust
let t = Some("Turing"); logicians.extend(t);
```

Advantages

- Small, idiomatic utilities for optional single-element flows.
