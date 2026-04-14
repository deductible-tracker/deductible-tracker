# 2.6 Finalisation in destructors

Description

Use `Drop` implementations and RAII guards to run cleanup code on scope exit (covering early returns and panics during unwind).

Example

```rust
struct Guard; impl Drop for Guard { fn drop(&mut self) { println!("exit"); } }
```

Advantages

- Ensures cleanup runs on normal return and unwinding (usually).

Disadvantages

- Destructors are not guaranteed in all abnormal termination scenarios; avoid panicking in `Drop`.
