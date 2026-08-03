// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the locked handle takes proven bytes directly, and a computed
//! value legitimately goes through formatting.

use std::io::{Write, stdout};

const GREETING: &[u8] = b"Hello, world!\n";

pub fn greet() -> std::io::Result<()> {
    stdout().lock().write_all(GREETING)
}

pub fn report(count: u32) {
    println!("processed {count}");
}
