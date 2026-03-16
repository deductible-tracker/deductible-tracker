# Deref-polymorphism

Description

Deref polymorphism is an anti-pattern when relied upon to achieve complex polymorphic behavior via `Deref` implementations. Overuse can hide ownership and make generic bounds fragile.

Problems

- Makes trait bounds and generic reasoning more complex.
- Can lead to surprising coercions and harder-to-debug code.

Alternatives

- Prefer explicit traits such as `AsRef`, `Borrow`, or custom traits to express conversion and borrowing intent.
