// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: `config` never changes inside the loop and the copy is only
//! read — one clone before the loop (or a borrow) would do.

#[derive(Clone)]
pub struct Config {
    pub retries: u32
}

pub fn total_retries(config: Config, jobs: &[u32]) -> u32 {
    let mut total = 0;
    for job in jobs {
        let snapshot = config.clone();
        total += snapshot.retries + job;
    }
    total
}
