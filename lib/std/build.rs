use std::{env, fs::File, io::Write, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Put the linker script somewhere the linker can find it. We can then specify the linker script with just
    // `link-arg=-Tlink.ld`.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    match env::var_os("TARGET").unwrap().to_str() {
        Some("x86_64-poplar") => {
            println!("cargo:rerun-if-changed=x64.x");
            File::create(out.join("link.ld")).unwrap().write_all(include_bytes!("x64.ld")).unwrap();
        }
        Some("riscv64gc-unknown-none-elf") => {
            println!("cargo:rerun-if-changed=rv64.x");
            File::create(out.join("link.ld")).unwrap().write_all(include_bytes!("rv64.ld")).unwrap();
        }
        _ => panic!("Building Poplar `std` for unsupported target!"),
    }
    println!("cargo:rustc-link-search={}", out.display());
}
