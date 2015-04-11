extern crate mmap;

use std::env;
use std::io::{self, Write};

use mmap::{Mmap, Protection};

/// Output a file's contents to stdout. The file path must be provided as the first process
/// argument.
fn main() {
    let path = env::args().nth(1).expect("supply a single path as the program argument");

    let mmap = Mmap::open(path, Protection::Read).unwrap();

    io::stdout().write_all(&mmap[..]).unwrap();
}
