fn main() {
    // Allows building the `kernel` crate host-side for unit testing
    if std::env::var("TARGET").unwrap() == "x86_64-unknown-none" {
        println!("cargo:rerun-if-changed=link.ld");
        println!("cargo:rustc-link-arg=-Tlink.ld");
    }
}
