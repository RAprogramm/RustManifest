// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where a value came from, in words.
//!
//! One phrase per node of the composed term, and nothing added between
//! them: a phrase says what the node is and names its parts, so the
//! sentence carries the shape of the derivation rather than a retelling
//! of it. A node the reading did not settle says which question it failed
//! on, in the same breath.

use crate::semantic::summary::ComposedOrigin;

/// The value one composed origin stands for, in words.
#[must_use]
pub fn origin_text(origin: &ComposedOrigin) -> String {
    match origin {
        ComposedOrigin::Written(literal) => format!("the written value {}", literal.quoted()),
        ComposedOrigin::Parameter(slot) => slot.name.as_ref().map_or_else(
            || format!("the value handed in at position {}", slot.position),
            |name| format!("the value handed in as `{name}`")
        ),
        ComposedOrigin::Field(key) => format!("the field `{key}`"),
        ComposedOrigin::Constant(named) => format!("the value `{named}`"),
        ComposedOrigin::Constructed(type_name) => format!("the `{type_name}` built here"),
        ComposedOrigin::Supplied(named) => {
            format!("the value handed to the closure as `{named}`")
        }
        ComposedOrigin::Beyond(candidate) => {
            format!("the value `{candidate}` hands back from outside this program")
        }
        ComposedOrigin::Boundary {
            channel,
            selector
        } => format!("{selector} of the {channel}"),
        ComposedOrigin::Computed {
            operation,
            operands
        } => {
            let named: Vec<String> = operands.iter().map(origin_text).collect();
            if named.is_empty() {
                format!("the result of {operation}")
            } else {
                format!("the result of {operation} over {}", listed(&named))
            }
        }
        ComposedOrigin::Choice(_) => {
            let mut named: Vec<String> = origin.flattened().iter().map(origin_text).collect();
            named.sort_unstable();
            named.dedup();
            match named.as_slice() {
                [] => "a value no alternative produces".to_owned(),
                [only] => only.clone(),
                several => format!("one of {}", listed(several))
            }
        }
        ComposedOrigin::Call {
            candidate,
            carried,
            returns,
            ..
        } => match carried.as_ref() {
            ComposedOrigin::NotDerived(gap) => returns.as_ref().map_or_else(
                || format!("the value `{candidate}` hands back ({gap})"),
                |named| format!("the `{named}` that `{candidate}` hands back ({gap})")
            ),
            settled => origin_text(settled)
        },
        ComposedOrigin::Leaves => "no value, because control leaves there".to_owned(),
        ComposedOrigin::NotDerived(gap) => format!("a value this analysis did not derive ({gap})")
    }
}

/// Several phrases as a list reads.
fn listed(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", "))
    }
}
