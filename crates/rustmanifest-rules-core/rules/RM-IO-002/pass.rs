// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the handle is locked once and the whole run writes through the
//! guard, and a single print needs no lock of its own.

use std::io::{Write, stdout};

pub fn banner() -> std::io::Result<()> {
    let mut out = stdout().lock();
    writeln!(out, "first")?;
    writeln!(out, "second")?;
    writeln!(out, "third")
}

pub fn once() {
    println!("only one line");
}
