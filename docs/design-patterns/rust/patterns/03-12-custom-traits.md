# Use custom traits to avoid complex type bounds

Description

When trait bounds become unwieldy (especially with `Fn` traits), introduce a named trait that
encapsulates the behaviour — implement it generically for closures/functions to simplify bounds.

Example

```rust
trait Getter { type Output: Display; fn get_value(&mut self) -> Result<Self::Output, Error>; }
impl<F: FnMut() -> Result<T, Error>, T: Display> Getter for F { type Output = T; fn get_value(&mut self) -> Result<Self::Output, Error> { self() } }
```

Last change: 2026-01-03, commit:f279f35
