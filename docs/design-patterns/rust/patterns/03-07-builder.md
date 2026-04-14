# Builder

Description

Construct an object with calls to a builder helper.

Example

```rust
#[derive(Debug, PartialEq)]
pub struct Foo { bar: String }
impl Foo { pub fn builder() -> FooBuilder { FooBuilder::default() } }
#[derive(Default)]
pub struct FooBuilder { bar: String }
impl FooBuilder {
    pub fn new() -> FooBuilder { FooBuilder { bar: String::from("X") } }
    pub fn name(mut self, bar: String) -> FooBuilder { self.bar = bar; self }
    pub fn build(self) -> Foo { Foo { bar: self.bar } }
}
```

Discussion

Useful when construction would otherwise require many constructors or side-effects. See `derive_builder` crate.

Last change: 2026-01-03, commit:f279f35
