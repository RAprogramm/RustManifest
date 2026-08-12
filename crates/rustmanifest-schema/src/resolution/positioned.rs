// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The one thing every settled row has in common: the place it speaks about.
//!
//! Four kinds of row travel in a resolution and all four are ordered, looked
//! up and de-duplicated the same way. Stating the common part once means the
//! ordering promise the shape makes is one implementation, so a fifth kind of
//! row cannot be added with an ordering of its own by accident.

use core::cmp::Ordering;

use crate::Location;

/// A settled row that names the place it speaks about.
pub(super) trait Positioned {
    /// File and byte range the row was read at.
    fn at(&self) -> &Location;

    /// What separates two rows settled at the very same place, so that
    /// ordering stays total and de-duplication cannot drop a distinct row.
    fn tiebreak(&self) -> &str;
}

/// Orders two rows by file, then by the range they cover, then by whatever
/// distinguishes rows sharing a place.
pub(super) fn order<T: Positioned>(left: &T, right: &T) -> Ordering {
    let (first, second) = (left.at(), right.at());
    first
        .file
        .cmp(&second.file)
        .then(first.range.start.cmp(&second.range.start))
        .then(first.range.end.cmp(&second.range.end))
        .then_with(|| left.tiebreak().cmp(right.tiebreak()))
}

/// Puts `rows` into the order the shape promises and drops exact repeats, so
/// two runs over the same workspace produce the same bytes.
pub(super) fn settle<T: Positioned + PartialEq>(rows: &mut Vec<T>) {
    rows.sort_by(order);
    rows.dedup();
}

/// Returns the first row settled at exactly `location`, or `None` where none
/// was.
///
/// The search is a bisection, which the number of rows a workspace produces
/// makes worth stating: a linear scan here is the difference between reading
/// a resolution and waiting for one. It answers correctly only on a settled
/// resolution — [`settle`] is what establishes the order it bisects.
pub(super) fn at<'a, T: Positioned>(rows: &'a [T], location: &Location) -> Option<&'a T> {
    let first = rows.partition_point(|row| {
        (&row.at().file, row.at().range.start, row.at().range.end)
            < (&location.file, location.range.start, location.range.end)
    });
    rows.get(first).filter(|row| row.at() == location)
}
