// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the loop writes into a buffered writer over the locked handle and
//! flushes once, so the syscall count follows the buffer, not the line count.

use std::io::{BufWriter, Write, stdout};

pub fn dump(lines: &[String]) -> std::io::Result<()> {
    let mut out = BufWriter::new(stdout().lock());
    for line in lines {
        writeln!(out, "{line}")?;
    }
    out.flush()
}
