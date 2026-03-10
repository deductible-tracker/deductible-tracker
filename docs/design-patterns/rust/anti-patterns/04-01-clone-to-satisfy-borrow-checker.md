# Clone to satisfy the borrow checker

Description

Cloning large values solely to satisfy borrow-checker constraints is an anti-pattern. Instead, prefer ownership-preserving patterns such as `mem::take`, `mem::replace`, or redesigning the ownership so cloning isn't required.

Why it's bad

- Extra allocations and runtime cost.
- Often indicates an API or ownership design issue.

Alternatives

- Use `mem::take` / `mem::replace` when moving out of place is necessary.
- Use `Option::take` for optional fields.
- Refactor to pass references where possible or return owned values on error to enable retries.
