# 2.11 Pass variables to closure

Description

Use local rebinding to control what a closure captures (move, clone, or borrow) and keep intent clear.

Example

```rust
let closure = { let num2 = num2.clone(); move || { *num1 + *num2 } };
```

Advantages

- Clear capture semantics and immediate drop of temporary values.
