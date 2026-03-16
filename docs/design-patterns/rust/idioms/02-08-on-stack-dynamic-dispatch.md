# 2.8 On-Stack Dynamic Dispatch

Description

Use `&mut dyn Trait` (or `&dyn Trait`) to avoid heap allocation while getting dynamic dispatch. Since Rust 1.79 lifetimes of temporaries in borrows are extended to be usable in this pattern.

Example

```rust
let readable: &mut dyn std::io::Read = if arg == "-" { &mut std::io::stdin() } else { &mut file };
```

Advantages

- No heap allocation; flexible runtime choice.

Disadvantages

- Dynamic dispatch overhead and lifetime subtleties before 1.79.
