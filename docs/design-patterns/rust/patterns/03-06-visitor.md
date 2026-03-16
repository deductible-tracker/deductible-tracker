# Visitor

Description

A visitor encapsulates an algorithm that operates over a heterogeneous collection of objects. It allows
multiple different algorithms to be written over the same data without having to modify the data (or
their primary behaviour).

Example

```rust
mod ast {
    pub enum Stmt { Expr(Expr), Let(Name, Expr) }
    pub struct Name { value: String }
    pub enum Expr { IntLit(i64), Add(Box<Expr>, Box<Expr>), Sub(Box<Expr>, Box<Expr>) }
}

mod visit {
    use super::ast::*;
    pub trait Visitor<T> {
        fn visit_name(&mut self, n: &Name) -> T;
        fn visit_stmt(&mut self, s: &Stmt) -> T;
        fn visit_expr(&mut self, e: &Expr) -> T;
    }
}

use ast::*;
use visit::*;

struct Interpreter;
impl Visitor<i64> for Interpreter {
    fn visit_name(&mut self, _n: &Name) -> i64 { panic!() }
    fn visit_stmt(&mut self, s: &Stmt) -> i64 {
        match *s {
            Stmt::Expr(ref e) => self.visit_expr(e),
            Stmt::Let(..) => unimplemented!(),
        }
    }
    fn visit_expr(&mut self, e: &Expr) -> i64 {
        match *e {
            Expr::IntLit(n) => n,
            Expr::Add(ref lhs, ref rhs) => self.visit_expr(lhs) + self.visit_expr(rhs),
            Expr::Sub(ref lhs, ref rhs) => self.visit_expr(lhs) - self.visit_expr(rhs),
        }
    }
}
```

Discussion

The visitor is useful when applying algorithms to heterogeneous data; `fold` is a related pattern that
produces a new data structure.

Last change: 2026-01-03, commit:f279f35
