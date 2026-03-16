# 2.15 Return consumed argument on error

Description

If a fallible function consumes an argument, return that argument inside the `Err` variant so callers can retry or try alternative strategies without cloning.

Example

```rust
pub fn send(value: String) -> Result<(), SendError> { Err(SendError(value)) }
```

Advantages

- Avoids clones for retry logic; better performance.

Disadvantages

- Slightly more complex error types.
