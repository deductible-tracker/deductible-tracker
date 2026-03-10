# Strategy (aka Policy)

Description

The Strategy design pattern is a technique that enables separation of concerns. It also allows to
decouple software modules through Dependency Inversion.

The basic idea behind the Strategy pattern is that, given an algorithm solving a particular problem, we
define only the skeleton of the algorithm at an abstract level, and we separate the specific algorithm’s
implementation into different parts.

Example

In this example our invariants (or abstractions) are `Formatter` and `Report`, while `Text` and `Json`
are our strategy structs. These strategies have to implement the `Formatter` trait.

```rust
use std::collections::HashMap;

type Data = HashMap<String, u32>;

trait Formatter {
    fn format(&self, data: &Data, buf: &mut String);
}

struct Report;

impl Report {
    fn generate<T: Formatter>(g: T, s: &mut String) {
        let mut data = HashMap::new();
        data.insert("one".to_string(), 1);
        data.insert("two".to_string(), 2);
        g.format(&data, s);
    }
}

struct Text;
impl Formatter for Text {
    fn format(&self, data: &Data, buf: &mut String) {
        for (k, v) in data {
            let entry = format!("{k} {v}\n");
            buf.push_str(&entry);
        }
    }
}

struct Json;
impl Formatter for Json {
    fn format(&self, data: &Data, buf: &mut String) {
        buf.push('[');
        for (k, v) in data.into_iter() {
            let entry = format!(r#"{{"{}":"{}"}}"#, k, v);
            buf.push_str(&entry);
            buf.push(',');
        }
        if !data.is_empty() { buf.pop(); }
        buf.push(']');
    }
}

fn main() {
    let mut s = String::from("");
    Report::generate(Text, &mut s);
    assert!(s.contains("one 1"));
    s.clear();
    Report::generate(Json, &mut s);
    assert!(s.contains(r#"{"one":"1"}"#));
}
```

Discussion

The main advantage is separation of concerns. The `serde` crate is a good example of strategy-like
customization by implementing traits such as `Serialize` and `Deserialize`.

Last change: 2026-01-03, commit:f279f35
