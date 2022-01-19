# Building a Rust `std` implementation for Poplar

### Steps
* Added a submodule `std::sys::poplar` for Poplar's platform-specifics. Included it when `target_os = "poplar"`.
* Add `target_os = poplar` to the list of platforms that empty `unix_ext` and `windows_ext` modules are created
  for, as I doubt it will compile properly. (I'm not sure if this is correct).
* Added all the boilerplate from the `unsupported` platform to Poplar's new module.
* In `std`'s, `build.rs`, add `target_os = "poplar"` to the list of platforms that have no special requirements.
  This means we're not a `restricted_std` platform.
* In `sys_common/mod.rs`, add `target_os = "poplar"` to the list of platforms that don't include the standard `sys_common/net` module.

### Making an entry point
The normal entry point of a Rust executable is actually provided by the system `libc` - the `crt0.o` file. We don't
want a `libc` equivalent on Poplar, so we need to find a way of defining one in the Rust `std`. This is easier than
it sounds - we just define a `#[no_mangle]` symbol called `_start` in Poplar's `sys` module, and it's linked into
the right place.
