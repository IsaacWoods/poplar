# Poplar's userspace
Poplar supports running programs in userspace on supporting architectures. This offers increased protection
and separation compared to running code in kernelspace - as a microkernel, Poplar tries to run as much code
in userspace as possible.

## Building a program for Poplar's userspace
Currently, the only officially supported language for writing userspace programs is Rust.

#### Target
Poplar provides custom target files for userspace programs. These are found in the `user/{arch}_poplar.toml` files.

#### Standard library
Poplar provides a Rust crate, called `std`, which replaces Rust's standard library. We've done this for a few
reasons:
 - We originally had targets and a `std` port in a fork of `rustc`. This proved difficult to maintain and required
     users to build a custom Rust fork and add it as a `rustup` toolchain. This is a high barrier of entry for
     anyone wanting to try Poplar out.
 - Poplar's ideal standard library probably won't end up looking very similar to other platform's, as there are
     significant ideological differences in how programs should interact with the OS. This is unfortunate from a
     porting point of view, but does allow us to design the platform interface from the group up.

The name of the crate is slightly unfortunate, but is required, as `rustc` uses the name of the crate to decide
where to import the prelude from. This significantly increases the ergonomics we can provide, so is worth the
tradeoff.

The `std` crate does a few important things that are worth understanding to reduce the 'magic' of Poplar's
userspace:
 - It provides a linker script - the linker script for the correct target is shipped as part of the crate, and
     then the build script copies it into the Cargo `OUT_DIR`. It also passes a directive to `rustc` such that
     you can simply pass `-Tlink.ld` to link with the correct script. This is, for example, done using `RUSTFLAGS`
     by Poplar's `xtask`, but you can also pass it manually or with another method, depending on your build system.
