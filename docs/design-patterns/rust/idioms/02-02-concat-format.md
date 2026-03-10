# 2.2 Concatenating strings with `format!`

Description

Use `format!` for readable string composition when combining literals and values. For heavy performance needs prefer `String::with_capacity` + `push_str`.

Example

```rust
fn say_hello(name: &str) -> String { format!("Hello {name}!") }
```

Advantages

- Readability and concision.

Disadvantages

- Slightly less efficient than manual `push` operations in tight loops.
