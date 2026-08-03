// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the entry returns the result of its own write, so a closed stream
//! leaves as an exit code instead of a panic. The file sits at the binary
//! root, so it is judged as a process entry point.

use std::io::{Write, stdout};

fn main() -> std::io::Result<()> {
    stdout().lock().write_all(b"Hello, world!\n")
}
