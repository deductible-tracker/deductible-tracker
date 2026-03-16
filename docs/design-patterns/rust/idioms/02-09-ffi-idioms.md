# 2.9 FFI Idioms

Description

When interfacing with C or other languages: keep `unsafe` blocks minimal, use `CStr`/`CString` for string conversions, and expose error codes or C-compatible structs for errors.

Example

```rust
let s: &str = unsafe { std::ffi::CStr::from_ptr(msg).to_str()? };
```

Advantages

- Safer, minimal `unsafe` usage and clear ownership expectations.

Disadvantages

- More boilerplate for FFI-safe types.
