# Pebble
[![Gitter chat](https://badges.gitter.im/gitterHQ/gitter.png)](https://gitter.im/pebble-os/Lobby)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![](https://tokei.rs/b1/github/Aaronepower/tokei)](https://github.com/pebble-os/pebble)
[![Build Status](https://travis-ci.org/pebble-os/pebble.svg?branch=master)](https://travis-ci.org/pebble-os/pebble)

Pebble is an operating system written in Rust, centered around message passing between 'nodes'.
It currently only supports x86_64 and is Multiboot2 compatible.

## Building
The microkernel currently builds with nightly `rustc 1.27.0-nightly 2018-04-29`. If it fails to build
for you with a later nightly, please file an issue!

1) Build the custom `rustc` and `libstd` with `cd rust; ./x.py build --target=x86_64-unknown-pebble` (NOT CURRENTLY NEEDED)
2) Build the kernel and package it into an ISO booted by GRUB2 with `make`
3) Start QEMU with `make qemu`

## Components
| Component                                                         | Description                                                           |
|-------------------------------------------------------------------|-----------------------------------------------------------------------|
| Kernel                                                            | The microkernel                                                       |
| [Our fork of Rust](https://github.com/pebble-os/rust)             | Additions to `rustc` and `libstd` for Pebble targets                  |
| [`libpebble`](https://github.com/pebble-os/libpebble)             | Library for interfacing with the kernel from userspace                |

## Acknowledgements
- [Phil Oppermann's great set of tutorials](https://os.phil-opp.com/)
- The OSDev [wiki](https://wiki.osdev.org/Main_Page) and [forums](https://forum.osdev.org)
- The Rust community at large
