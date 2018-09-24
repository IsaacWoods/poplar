///! `cargo-pebble` is a Cargo subcommand for building Pebble distributions. It allows you to
///! easily set up a suitable environment for building, build a customized kernel and set of
///! components, and package them into an image that can be booted using an emulator or on real
///! hardware.
///!
///! `cargo-pebble` uses a JSON configuration file to specify the whole build process. The aim is
///! to guarantee reproducible builds - if the configuration file and `rustc` versions are the same
///! between builds, the produced Pebble images should be.

fn main() {
    println!("Hello, world!");
}
