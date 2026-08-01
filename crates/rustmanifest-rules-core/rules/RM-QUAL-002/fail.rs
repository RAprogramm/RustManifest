// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the doc says "Returns", the body increments state.

pub struct Counter {
    pub hits: u64
}

impl Counter {
    /// Returns the hit count.
    pub fn hits(&mut self) -> u64 {
        self.hits += 1;
        self.hits
    }
}
