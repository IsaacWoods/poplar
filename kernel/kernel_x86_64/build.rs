fn main() {
    println!("cargo:rerun-if-changed=kernel_x86_64/link.ld");
    println!("cargo:rustc-link-arg=-Tkernel_x86_64/link.ld");
}
