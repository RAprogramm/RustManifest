// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the standard output stream is line-buffered, so each iteration
//! issues its own write syscall.

pub fn dump(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}
