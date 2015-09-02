# memmap

A Rust library for cross-platform memory-mapped file IO.

[Documentation](https://danburkert.github.io/memmap-rs/memmap/index.html)

[![Linux Status](https://travis-ci.org/danburkert/memmap-rs.svg?branch=master)](https://travis-ci.org/danburkert/memmap-rs)
[![Build status](https://ci.appveyor.com/api/projects/status/ubka00959pstatkg/branch/master?svg=true)](https://ci.appveyor.com/project/danburkert/mmap)

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

## License

`memmap` is primarily distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.

Copyright (c) 2015 Dan Burkert.

