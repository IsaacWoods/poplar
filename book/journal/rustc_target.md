# Building a `rustc` target for Pebble
We want a target in `rustc` for building userspace programs for Pebble. It would be especially cool to get it
merged as an upstream Tier-3 target. This documents my progress, mainly as a reference for me to remember how it all
works.

### How do I actually build and use `rustc`?
A useful baseline invocation for normal use is:
```
./x.py build -i library/std
```

The easiest way to test the built `rustc` is to create a `rustup` toolchain (from the root of the Rust repo):
```
rustup toolchain link pebble build/{host triple}/stage1     # If you built a stage-1 compiler (default with invocation above)
rustup toolchain link pebble build/{host triple}/stage2     # If you built a stage-2 compiler
```
It's easiest to call your toolchain `pebble`, as this is the name we use in the Makefiles for now.

You can then use this toolchain from Cargo anywhere on the system with:
```
cargo +pebble build     # Or whatever command you need
```

### Using a custom LLVM
- Fork Rust's `llvm-project`
- `cd src/llvm_project`
- `git remote add my_fork {url to your custom LLVM's repo}`
- `git fetch my_fork`
- `git checkout my_fork/{the correct branch}`
- `cd ..`
- `git add llvm-project`
- `git commit -m "Move to custom LLVM"`

### Things to change in `config.toml`
This is as of `2020-09-29` - you need to remember to keep the `config.toml` up-to-date (as it's not checked in
upstream), and can cause confusing errors when it's out-of-date.

- `download-ci-llvm = true` under `[llvm]`. This makes the build much faster, since we don't need a custom LLVM.
- `assertions = true` under `[llvm]`
- `incremental = true` under `[rust]`
- `lld = true` under `[rust]`. Without this, the toolchain can't find `rust-lld` when linking.
- `llvm-tools = true` under `[rust]`. This probably isn't needed, I just chucked it in in case `rust-lld` needs it.

### Adding the target
I used a slightly different layout to most targets (which have a base, which creates a `TargetOptions`, and then a
target that modifies and uses those options).

- Pebble targets generally need a custom linker script. I added one at `compiler/rustc_target/src/spec/x86_64_pebble.ld`.
- Make a module for the target (I called mine `compiler/rustc_target/src/spec/x86_64_pebble.rs`). Copy from a
  existing one. Instead of a separate `pebble_base.rs` to create the `TargetOptions`, we do it in the target
  itself. We `include_str!` the linker script in here, so it's distributed as part of the `rustc` binary.
- Add the target in the `supported_targets!` macro in `compiler/rustc_target/src/spec/mod.rs`.

### Adding the target to LLVM
I don't really know my way around the LLVM code base, so this was fairly cobbled together:
- In `llvm/include/llvm/ADT/Triple.h`, add a variant for the OS in the `OSType` enum. I called it `Pebble`. Don't
  make it the last entry, to avoid having to change the `LastOSType` variant.
- In `llvm/lib/Support/Triple.cpp`, in the function `Triple::getOSTypeName`, add the OS. I added `case Pebble: return "pebble";`.
- In the same file, in the `parseOS` function, add the OS. I added `.StartsWith("pebble", Triple::Pebble)`.
- This file also contains a function, `getDefaultFormat`, that gives the default format for a platform. The default
  is ELF, so no changes were needed for Pebble, but they might be for another OS.

TIP: When you make a change in the `llvm-project` submodule, you will need to commit these changes, and then update
the submodule in the parent repo, or the bootstrap script will checkout the old version (without your changes) and
build an entire compiler without the changes you are trying to test.

NOTE: to avoid people from having to switch to our `llvm-project` fork, we don't actually use our LLVM target from
`rustc` (yet). I'm not sure why you need per-OS targets in LLVM, as it doesn't even seem to let us do any of the
things we wanted to (this totally might just be me not knowing how LLVM targets work).

### Notes
- We needed to change the entry point to `_start`, or it silently just doesn't emit any sections in the final
  image.
- By default, it throws away our `.caps` sections. We need a way to emit it regardless - this is done by manually
  creating the program header and specifying that they should be kept with `KEEP`. There are two possible solutions
  that I can see: make `rustc` emit a linker script, or try and introduce these ideas into `llvm`/`lld` with our
  target (I'm not even sure this is possible).
- It looks like `lld` has no OS-specific code at all, and the only place that specifically-kept sections are added
  is in the linker script parser. Looks like we might have to actually create a file-based linker script (does
  literally noone else need to pass a linker script by command line??).
