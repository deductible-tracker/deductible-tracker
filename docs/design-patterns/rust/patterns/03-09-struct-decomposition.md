# Struct decomposition for independent borrowing

Description

Sometimes a large struct will cause issues with the borrow checker — decomposing the struct into
smaller structs and composing them can allow independent borrowing of parts.

Example

```rust
#[derive(Debug, Clone)] struct ConnectionString(String);
#[derive(Debug, Clone, Copy)] struct Timeout(u32);
#[derive(Debug, Clone, Copy)] struct PoolSize(u32);
struct Database { connection_string: ConnectionString, timeout: Timeout, pool_size: PoolSize }
```

Discussion

This pattern works around borrow-checker limitations and often reveals better abstractions.

Last change: 2026-01-03, commit:f279f35
