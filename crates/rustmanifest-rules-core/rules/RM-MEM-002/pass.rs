// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: cloning once before the loop, cloning the loop's own item, and
//! cloning a copy that each iteration mutates are all fine.

#[derive(Clone)]
pub struct Config {
    pub retries: u32
}

pub fn total_retries(config: Config, jobs: &[u32]) -> u32 {
    let snapshot = config.clone();
    let mut total = 0;
    for job in jobs {
        total += snapshot.retries + job;
    }
    total
}

pub fn keep_all(config: Config, jobs: &[u32]) -> Vec<Config> {
    let mut kept = Vec::with_capacity(jobs.len());
    for _job in jobs {
        let snapshot = config.clone();
        kept.push(snapshot);
    }
    kept
}

pub fn bump_each(config: Config, jobs: &[u32]) -> u32 {
    let mut last = 0;
    for _job in jobs {
        let mut snapshot = config.clone();
        snapshot.retries += 1;
        last = snapshot.retries;
    }
    last
}
