// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the text is proven at analysis time, yet every character travels
//! through format arguments before reaching the stream.

pub fn greet() {
    println!("Hello, world!");
}
