// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the loop writes into a buffered writer over the locked handle and
//! flushes once, so the syscall count follows the buffer, not the line count.
//!
//! `summarize` carries the fixture's discrimination: it holds a write the
//! engine does record, in a body that also holds a loop, with the write
//! standing outside that loop. A check that reported any recorded write in a
//! body containing a loop — or any recorded write at all — would fire here.

use std::io::{BufWriter, Write, stdout};

pub fn dump(lines: &[String]) -> std::io::Result<()> {
    let mut out = BufWriter::new(stdout().lock());
    for line in lines {
        writeln!(out, "{line}")?;
    }
    out.flush()
}

pub fn summarize(lines: &[String]) {
    let mut total = 0usize;
    for line in lines {
        total += line.len();
    }
    println!("{total} byte(s) across {} line(s)", lines.len());
}
