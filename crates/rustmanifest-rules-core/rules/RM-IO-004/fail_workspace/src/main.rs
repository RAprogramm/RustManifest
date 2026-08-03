// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the entry returns nothing while printing, so a broken pipe aborts
//! the process through a panic. The file sits at the binary root, which is
//! what makes it a process entry point at all.

fn main() {
    println!("Hello, world!");
}
