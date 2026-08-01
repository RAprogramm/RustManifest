// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: every private function is mentioned by a call.

pub fn used() {
    helper();
}

fn helper() {}
