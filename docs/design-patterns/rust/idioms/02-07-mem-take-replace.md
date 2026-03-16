# 2.7 `mem::take` / `mem::replace` to keep owned values in changed enums

Description

Use `std::mem::take` or `mem::replace` to move an owned field out of a mutable reference (e.g., switching enum variants) without cloning.

Example

```rust
use std::mem;
*e = MyEnum::B { name: mem::take(name) };
```

Advantages

- Avoids unnecessary allocation and cloning.

Disadvantages

- Requires `Default` for `mem::take`; otherwise use `mem::replace`.
