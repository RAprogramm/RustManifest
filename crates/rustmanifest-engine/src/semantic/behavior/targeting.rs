// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type a conversion produces, where the function it stands in hands
//! that very value back.
//!
//! A conversion whose target the caller writes nowhere is unidentified
//! only until the function's own signature answers for it: where the
//! value the conversion produces is the value the function hands back,
//! the declared return type is the target, and reading it there is a fact
//! about this function rather than a guess about a consumer.
//!
//! The distinction matters. Reading the target off the type a *caller*
//! declares for the value it receives would state a fact about the
//! caller; reading it off the signature the conversion's own function
//! declares states a fact about that function, which is the one the
//! source wrote.
//!
//! Only a position handing the value back outright answers. A value that
//! passes through anything the reading cannot see leaves the target
//! unidentified, and the claim says so.

use super::{types::FunctionFacts, value::Provenance};
use crate::semantic::behavior::operation::{Conversion, Operation};

/// The type a rendered return hands back to a caller that unwraps it:
/// what a fallible or optional return carries, and the return itself
/// otherwise.
fn handed_back(rendered: &str) -> Option<String> {
    let trimmed = rendered.trim();
    let outermost = crate::semantic::behavior::outermost_name(trimmed)?;
    if !matches!(outermost.as_str(), "Result" | "Option") {
        return Some(outermost);
    }
    let inside = trimmed
        .split_once('<')
        .and_then(|(_, rest)| rest.strip_suffix('>'))?;
    let carried = inside.split(',').next()?;
    crate::semantic::behavior::outermost_name(carried)
}

/// Names the target of every conversion whose value this function hands
/// back, from the return the compiler rendered for it.
pub(super) fn name_targets(facts: &mut FunctionFacts, rendered: Option<&str>) {
    let Some(declared) = rendered.and_then(handed_back) else {
        return;
    };
    if declared.is_empty() {
        return;
    }
    let handed_back: Vec<u32> = facts
        .handbacks
        .iter()
        .filter_map(|position| match position.value {
            Provenance::Computed(index) => Some(index),
            _ => None
        })
        .collect();
    for index in handed_back {
        let Some(site) = usize::try_from(index)
            .ok()
            .and_then(|at| facts.operations.get_mut(at))
        else {
            continue;
        };
        let Operation::Conversion(conversion) = &site.operation else {
            continue;
        };
        if conversion.target.is_some() {
            continue;
        }
        site.operation = Operation::Conversion(Conversion {
            target: Some(declared.clone())
        });
        site.produced = Some(declared.clone());
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        semantic::{
            behavior::{Operation, behavior_with},
            settling::Settling
        },
        source::Source
    };

    /// The target of the first conversion `function` performs, read with
    /// the type a resolver settled for the value being converted.
    ///
    /// Which library method a call reaches is decided by the type it is
    /// written on, and that type is the compiler's answer; the fixture
    /// states it where the body writes the receiver.
    fn target(text: &str, function: &str) -> Option<String> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = Settling::over("src/lib.rs", text)
            .after("->")
            .typed("text", "&str")
            .done();
        behavior_with(&sources, Some(&resolution))
            .iter()
            .find(|fact| fact.function == function)
            .and_then(|fact| {
                fact.operations
                    .iter()
                    .find_map(|site| match &site.operation {
                        Operation::Conversion(conversion) => Some(conversion.target.clone()),
                        _ => None
                    })
            })
            .flatten()
    }

    /// A conversion whose value the function hands back takes its target
    /// from that function's own declared return.
    #[test]
    fn a_handed_back_conversion_takes_the_declared_return_as_its_target() {
        assert_eq!(
            target(
                "fn number(text: &str) -> Result<f64, Fault> { text.parse().map_err(|_| Fault) }",
                "number"
            )
            .as_deref(),
            Some("f64")
        );
    }

    /// A conversion whose value goes anywhere else keeps its target
    /// unidentified: this reading answers for the function's own return
    /// and for nothing else.
    #[test]
    fn a_conversion_going_elsewhere_stays_unidentified() {
        assert_eq!(
            target(
                "fn number(text: &str) -> Result<f64, Fault> { let value = text.parse(); \
                 Ok(0.0) }",
                "number"
            ),
            None
        );
    }
}
