// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which wire shapes are filled from the records the program keeps.
//!
//! A shape assembled for the wire and a store held in memory are two
//! sentences until a derivation ties them: the value written into the
//! shape's collection field traces back, through the calls it passes,
//! to a read of the kept store's own field. Only that trace makes the
//! claim — two facts in one body prove co-occurrence, and co-occurrence
//! is not a filling.

use super::store::{Store, stores};
use crate::semantic::{summary::ComposedOrigin, workspace::Workspace};

/// One wire shape filled from one kept store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feeding {
    /// The wire shape's collection, as `Owner.field`.
    pub wire:     String,
    /// The kept store it is filled from, as `Owner.field`.
    pub from:     String,
    /// Function whose body performs the filling.
    pub function: String
}

/// Every wire shape provably filled from a kept store.
#[must_use]
pub fn feedings(workspace: &Workspace<'_>) -> Vec<Feeding> {
    let held = stores(workspace);
    let (wired, kept): (Vec<&Store>, Vec<&Store>) = held.iter().partition(|store| store.wire);
    if wired.is_empty() || kept.is_empty() {
        return Vec::new();
    }
    let sources = workspace.sources();
    let facts = workspace.facts();
    let qualifiers = crate::semantic::meaning::qualifiers_by_fact(sources, facts);
    let composed = crate::semantic::summary::composition_with(
        sources,
        workspace.resolution(),
        facts,
        workspace.calls(),
        &qualifiers
    );
    let mut fed = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        for built in &fact.built_fields {
            for wire in wired
                .iter()
                .filter(|store| store.owner == built.owner && store.field == built.field)
            {
                let term = composed.of(index, &built.origin);
                for keeper in &kept {
                    let key = format!("{}.{}", keeper.owner, keeper.field);
                    if mentions_field(&term, &key) {
                        fed.push(Feeding {
                            wire:     format!("{}.{}", wire.owner, wire.field),
                            from:     key,
                            function: fact.function.clone()
                        });
                    }
                }
            }
        }
    }
    fed.sort_unstable_by(|left, right| {
        (&left.wire, &left.from, &left.function).cmp(&(&right.wire, &right.from, &right.function))
    });
    fed.dedup();
    fed
}

/// Whether a derivation tree reaches the value of `key` anywhere: as
/// the term itself, as an argument or answer of a call it passes
/// through, as one alternative of a choice, or as an operand of a
/// computation.
fn mentions_field(term: &ComposedOrigin, key: &str) -> bool {
    match term {
        ComposedOrigin::Field(named) => named == key,
        ComposedOrigin::Call {
            arguments,
            carried,
            failed,
            ..
        } => {
            arguments
                .iter()
                .any(|argument| mentions_field(argument, key))
                || mentions_field(carried, key)
                || mentions_field(failed, key)
        }
        ComposedOrigin::Choice(alternatives) => alternatives
            .iter()
            .any(|alternative| mentions_field(alternative, key)),
        ComposedOrigin::Computed {
            operands, ..
        } => operands.iter().any(|operand| mentions_field(operand, key)),
        _ => false
    }
}
