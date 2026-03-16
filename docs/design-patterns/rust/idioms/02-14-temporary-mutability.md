# 2.14 Temporary mutability

Description

When data must be mutated during setup but then used immutably, rebind the variable as immutable or use a nested block to make intent explicit.

Example

```rust
let mut data = get_vec(); data.sort(); let data = data; // now immutable
```

Advantages

- Prevents accidental mutation after initialization.
