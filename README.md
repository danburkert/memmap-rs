# memmap2
[![Build Status](https://travis-ci.org/RazrFalcon/memmap2-rs.svg?branch=master)](https://travis-ci.org/RazrFalcon/memmap2-rs)
[![Windows Build status](https://ci.appveyor.com/api/projects/status/3518plsu6mutb07q/branch/master?svg=true)](https://ci.appveyor.com/project/RazrFalcon/memmap2-rs)
[![Crate](https://img.shields.io/crates/v/memmap2.svg)](https://crates.io/crates/memmap2)
[![Documentation](https://docs.rs/memmap2/badge.svg)](https://docs.rs/memmap2)
[![Rust 1.13+](https://img.shields.io/badge/rust-1.13+-orange.svg)](https://www.rust-lang.org)

A Rust library for cross-platform memory mapped IO.

This is a **fork** of the [memmap-rs](https://github.com/danburkert/memmap-rs) crate.

## Changes

- Remove `winapi` dependency. [memmap-rs/pull/89](https://github.com/danburkert/memmap-rs/pull/89)
- Use `LICENSE-APACHE` instead of `README.md` for some tests since it's immutable.

## Features

- [x] file-backed memory maps
- [x] anonymous memory maps
- [x] synchronous and asynchronous flushing
- [x] copy-on-write memory maps
- [x] read-only memory maps
- [x] stack support (`MAP_STACK` on unix)
- [x] executable memory maps
- [ ] huge page support

## Platforms

`memmap2` should work on any platform supported by
[`libc`](https://github.com/rust-lang-nursery/libc#platforms-and-documentation).
`memmap2` requires Rust stable 1.13 or greater.

`memmap2` is continuously tested on:
  * `x86_64-unknown-linux-gnu` (Linux)
  * `i686-unknown-linux-gnu`
  * `x86_64-unknown-linux-musl` (Linux MUSL)
  * `x86_64-apple-darwin` (OSX)
  * `i686-apple-darwin`
  * `x86_64-pc-windows-msvc` (Windows)
  * `i686-pc-windows-msvc`
  * `x86_64-pc-windows-gnu`
  * `i686-pc-windows-gnu`

`memmap2` is continuously cross-compiled against:
  * `arm-linux-androideabi` (Android)
  * `aarch64-unknown-linux-gnu` (ARM)
  * `arm-unknown-linux-gnueabihf`
  * `mips-unknown-linux-gnu` (MIPS)
  * `x86_64-apple-ios` (iOS)
  * `i686-apple-ios`

## License

`memmap2` is primarily distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.

Copyright (c) 2015 Dan Burkert.
