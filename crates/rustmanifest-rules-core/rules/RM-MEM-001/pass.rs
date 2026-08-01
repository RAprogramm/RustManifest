// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: borrowing for reads, taking ownership only when the value is
//! actually consumed, and `Copy` types by value are all fine.

#[derive(Clone, Copy)]
pub struct Coord {
    pub x: u8
}

pub struct Report {
    pub total: u64
}

pub fn summarize(report: &Report) -> u64 {
    report.total
}

pub fn consume(report: Report) -> Report {
    report
}

pub fn dx(coord: Coord) -> u8 {
    coord.x
}
