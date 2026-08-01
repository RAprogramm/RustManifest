// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: the name says "read", the body increments state and logs —
//! the promise and the behavior contradict.

pub struct Counter {
    pub hits: u64
}

impl Counter {
    pub fn get_hits(&mut self) -> u64 {
        self.hits += 1;
        println!("counted");
        self.hits
    }
}
