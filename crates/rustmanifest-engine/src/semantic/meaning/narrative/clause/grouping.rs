// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Gathering the sites of one clause by the function that holds them.
//!
//! A clause covering several functions cannot honestly speak of them at
//! once. Certainty belongs to a site and reaches a group only through the
//! function holding it: one function reached unconditionally and another
//! reached by no chain at all share no grade, and stating the weaker of
//! the two over both says of the certain one what is true only of the
//! other.
//!
//! Grouping is by the function's own name, and the groups keep the order
//! their first site was recorded in, so a statement reads in the order the
//! facts were found rather than in an order chosen for it.

use crate::semantic::behavior::Grade;

/// The sites of one function, with how certainly they run.
pub(super) struct Held<'claim, T> {
    /// Function holding them.
    pub(super) function: &'claim str,
    /// How certainly the least certain of them runs.
    pub(super) grade:    Grade,
    /// The sites themselves, in recorded order.
    pub(super) sites:    Vec<&'claim T>
}

/// Gathers claims by the function holding them, keeping first-seen order.
pub(super) fn by_function<'claim, T>(
    claims: &'claim [T],
    named: impl Fn(&'claim T) -> &'claim str,
    graded: impl Fn(&'claim T) -> Grade
) -> Vec<Held<'claim, T>> {
    let mut groups: Vec<Held<'claim, T>> = Vec::new();
    for claim in claims {
        let function = named(claim);
        let grade = graded(claim);
        match groups.iter_mut().find(|held| held.function == function) {
            Some(held) => {
                held.grade = held.grade.min(grade);
                held.sites.push(claim);
            }
            None => groups.push(Held {
                function,
                grade,
                sites: vec![claim]
            })
        }
    }
    groups
}

/// Joins the parts of one clause into a sentence, as a list reads.
pub(super) fn joined(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", "))
    }
}
