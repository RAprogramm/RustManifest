// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the entry returns nothing while printing, so a broken pipe aborts
//! the process through a panic.

fn main() {
    println!("Hello, world!");
}
