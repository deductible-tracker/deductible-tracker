# Newtype

Description

Use a tuple struct with a single field to make an opaque wrapper for a type. This creates a new type, rather than an alias to a type.

Example

```rust
use std::fmt::Display;

// Create Newtype Password to override the Display trait for String
struct Password(String);

impl Display for Password {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "****************")
	}
}

fn main() {
	let unsecured_password: String = "ThisIsMyPassword".to_string();
	let secured_password: Password = Password(unsecured_password.clone());
	println!("unsecured_password: {}", unsecured_password);
	println!("secured_password: {}", secured_password);
}
```

Motivation

The primary motivation for newtypes is abstraction. It allows you to share implementation details between types while precisely controlling the interface. By using a newtype rather than exposing the implementation type as part of an API, it allows you to change implementation backwards compatibly.

Newtypes can be used for distinguishing units, e.g., wrapping `f64` to give distinguishable `Miles` and `Kilometres`.

Advantages

- The wrapped and wrapper types are not type compatible (as opposed to using `type`), so users of the newtype will never 'confuse' the wrapped and wrapper types.
- Newtypes are a zero-cost abstraction - there is no runtime overhead.
- The privacy system ensures that users cannot access the wrapped type (if the field is private, which it is by default).

Disadvantages

- There is no special language support. This means there can be a lot of boilerplate. You need a 'pass through' method for every method you want to expose on the wrapped type, and an impl for every trait you want to also be implemented for the wrapper type.

Discussion

Newtypes are very common in Rust code. Abstraction or representing units are the most common use-cases. They enable encapsulation and safer APIs without runtime cost.
