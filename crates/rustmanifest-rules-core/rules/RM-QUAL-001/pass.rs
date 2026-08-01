// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: honest getters observe only; mutators say what they do; the
//! `_mut` suffix withdraws the read promise explicitly.

pub struct Counter {
    pub hits: u64
}

impl Counter {
    pub fn get_hits(&self) -> u64 {
        self.hits
    }

    pub fn get_hits_mut(&mut self) -> &mut u64 {
        &mut self.hits
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
    }
}
