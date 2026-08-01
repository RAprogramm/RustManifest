// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: `line` is allocated fresh on every iteration and never leaves
//! it — the buffer belongs outside the loop, cleared per pass.

pub fn total_width(values: &[u32]) -> usize {
    let mut width = 0;
    for _value in values {
        let mut line = String::with_capacity(16);
        line.push('#');
        width += line.len();
    }
    width
}
