// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: three prints in one body take the stream's reentrant lock three
//! times where one acquisition would serve the run.

pub fn banner() {
    println!("first");
    println!("second");
    println!("third");
}
