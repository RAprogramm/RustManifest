// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the handle is locked once and the whole run writes through the
//! guard, and a single print needs no lock of its own.
//!
//! The last two functions carry the fixture's discrimination, each with
//! writes the engine does record: `note` writes once to each standard stream,
//! which are two independent locks and therefore no run at all, and
//! `describe` writes twice to one stream from branches that exclude each
//! other, so exactly one acquisition happens on any execution.

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

pub fn note(problem: &str) {
    println!("checked");
    eprintln!("{problem}");
}

pub fn describe(ok: bool) {
    if ok {
        println!("ok");
    } else {
        println!("failed");
    }
}
