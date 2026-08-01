// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: `summarize` takes `Report` by value but only reads a field,
//! so callers lose ownership for nothing.

pub struct Report {
    pub total: u64
}

pub fn summarize(report: Report) -> u64 {
    report.total
}
