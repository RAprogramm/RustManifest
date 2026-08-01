// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture crate root: `stale` is private and never mentioned.

pub fn used() {
    helper();
}

fn helper() {}

fn stale() {}
