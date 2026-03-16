# 2.5 Collections are smart pointers (Deref)

Description

Collections implement `Deref` to provide borrowed views (`Vec<T>` -> `[T]`, `String` -> `str`). Implement methods on the borrowed view where appropriate.

Advantages

- Ergonomic APIs and implicit coercions.

Disadvantages

- Can complicate generic bounds; prefer explicit traits (`AsRef`, `Borrow`) when needed.
