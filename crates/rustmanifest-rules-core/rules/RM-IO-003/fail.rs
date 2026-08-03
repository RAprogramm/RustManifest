// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the standard output stream is line-buffered and the standard
//! error stream is not buffered at all, so each iteration of either loop
//! issues its own write syscall.

pub fn dump(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

pub fn warn(problems: &[String]) {
    for problem in problems {
        eprintln!("{problem}");
    }
}
