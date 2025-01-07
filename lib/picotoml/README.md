# `picotoml`
A `no_std` TOML deserializer for `serde`. Forked from [`TroyNeubauer/minimal-toml`](https://github.com/TroyNeubauer/minimal-toml),
which has not been published on crates.io, plus elements of its fork of its `peekmore` dependency.

## Example Usage

Cargo.toml: 
```toml
[dependencies]
picotoml = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
```

main.rs: 
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MyConfig {
    pub app_name: String,
    pub version: u32,
    pub enable: bool,
    pub options: Option<String>,

    pub users: Vec<String>,
    pub scores: Vec<i32>,

    pub server: Server,
}

#[derive(Debug, Deserialize)]
pub struct Server {
    pub ip: String,
    pub port: u16,
}

const MY_CONFIG_TOML: &str = r#"
app_name = "MyApp"
version = 1
enable = true
options = "SomeOption"

users = ["Alice", "Bob", "Charlie"]
scores = [100, 200, 300]

[server]
ip = "127.0.0.1"
port = 8080
"#;

fn main() {
    let config = picotoml::from_str::<MyConfig>(MY_CONFIG_TOML).unwrap();
    dbg!(config);
}
```

This will output:
```
[src/main.rs:38:5] config = MyConfig {
    app_name: "MyApp",
    version: 1,
    enable: true,
    options: Some(
        "SomeOption",
    ),
    users: [
        "Alice",
        "Bob",
        "Charlie",
    ],
    scores: [
        100,
        200,
        300,
    ],
    server: Server {
        ip: "127.0.0.1",
        port: 8080,
    },
}
```
