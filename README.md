# memmap

cross-platform Rust API for memory-mapped file IO.

[rustdoc](https://danburkert.github.io/memmap-rs/memmap/index.html)

[![Linux build Status](https://travis-ci.org/danburkert/memmap-rs.svg?branch=master)](https://travis-ci.org/danburkert/memmap-rs)
[![Windows build status](https://ci.appveyor.com/api/projects/status/ubka00959pstatkg?svg=true)](https://ci.appveyor.com/project/danburkert/mmap)


## Features

- [x] POSIX support
- [x] Windows support
- [x] file-backed memory maps
- [x] anonymous memory maps
- [x] synchronous and asynchrounous flushing
- [x] copy-on-write memory maps
- [x] read-only memory maps
- [x] stack support (`MAP_STACK` on unix)
- [ ] huge page support
