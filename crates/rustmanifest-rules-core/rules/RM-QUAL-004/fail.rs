// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: `stale` is private and nothing anywhere mentions it.

pub fn used() {
    helper();
}

fn helper() {}

fn stale() {}
