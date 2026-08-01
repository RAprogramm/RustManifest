// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: a buffer hoisted out of the loop and cleared per pass, and a
//! buffer that is moved into the result each iteration, are both fine.

pub fn total_width(values: &[u32]) -> usize {
    let mut width = 0;
    let mut line = String::with_capacity(16);
    for _value in values {
        line.clear();
        line.push('#');
        width += line.len();
    }
    width
}

pub fn render_all(values: &[u32]) -> Vec<String> {
    let mut lines = Vec::with_capacity(values.len());
    for value in values {
        let mut line = String::new();
        line.push_str(&value.to_string());
        lines.push(line);
    }
    lines
}
