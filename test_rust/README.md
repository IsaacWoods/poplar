# Test Rust Program
This is a tiny little program to test the `std` implementation for Pebble.
To use the correct toolchain, it's easiest to use `rustup`:
```
rustup toolchain link pebble-toolchain ../rust/build/x86_64-unknown-linux-gnu/stage2/
rustup override set pebble-toolchain
```

You can then build using: `cargo build --target=x86_64-unknown-pebble`
