// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: docs tell the truth about reading and about mutating.

pub struct Counter {
    pub hits: u64
}

impl Counter {
    /// Returns the hit count.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Increments the hit count.
    pub fn bump(&mut self) {
        self.hits += 1;
    }
}
