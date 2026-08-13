// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The deduction chain: the machine's own reasoning from an intent to
//! an installed block, one recorded fact per step.
//!
//! A chain is not prose about the code — it is the plan itself. Every
//! step is a claim the tables can answer for, in order: the type is
//! declared and holds named fields; each field provably carries the
//! wanted ability, by the same proof the canon bodies are written
//! from; the ability is not already given; the canonical form is
//! chosen by the catalog; the montage installs it; and the re-reading
//! confirms the gain, panic-freedom included. A chain that fails
//! carries its steps up to the failing claim and installs nothing —
//! the refusal is as auditable as the success.

use rustmanifest_schema::TraceStep;

use super::{
    emitted::{Gain, install_impl},
    graph::{TypeGraph, TypeKind},
    implement::canon::field_carries,
    why::Answer
};
use crate::source::Source;

/// One finished deduction: the chain, and the confirmed gain where the
/// chain reached installation.
#[derive(Debug, Clone)]
pub struct Deduction {
    /// The reasoning, one recorded fact per step.
    pub chain:   Answer,
    /// The confirmed gain, present only when every step held.
    pub gain:    Option<Gain>,
    /// The structural marker of the catalog's choice, present where the
    /// chain reached one.
    pub catalog: Option<CatalogChoice>
}

/// The catalog's choice as data: the step stating it, and the form.
///
/// An adviser reads this marker instead of parsing the step's prose —
/// a changed wording must not silently drop anyone's advice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogChoice {
    /// Index of the chain step stating the choice.
    pub step: usize,
    /// The chosen form's catalog name.
    pub form: String
}

/// Deduces, installs, and confirms the impl of `trait_name` for
/// `type_name`, or stops the chain at the first claim that fails.
#[must_use]
pub fn deduce_impl(
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>,
    file: usize,
    type_name: &str,
    trait_name: &str
) -> Deduction {
    let mut types = TypeGraph::from_sources(sources);
    types.granted_by(sources, settled);
    let mut steps = Vec::new();
    let Some(node) = types.type_named(type_name) else {
        return stopped(steps, format!("no type named `{type_name}` is declared"));
    };
    steps.push(TraceStep {
        fact:   format!(
            "`{type_name}` is a declared {} with {} recorded field(s)",
            match node.kind {
                TypeKind::Struct => "struct",
                TypeKind::Enum => "enum"
            },
            node.fields.len()
        ),
        reason: "the type graph holds its definition".to_owned()
    });
    if node.kind != TypeKind::Struct || node.fields.is_empty() {
        return stopped(
            steps,
            "the canon writes bodies only for structs with named fields".to_owned()
        );
    }
    for field in &node.fields {
        if field_carries(&types, &field.type_name, trait_name) {
            steps.push(TraceStep {
                fact:   format!(
                    "`{}: {}` provably carries `{trait_name}`",
                    field.name, field.type_name
                ),
                reason: "the std tables and the workspace impls answer for it".to_owned()
            });
        } else {
            steps.push(TraceStep {
                fact:   format!(
                    "`{}: {}` does not provably carry `{trait_name}`",
                    field.name, field.type_name
                ),
                reason: "neither the std tables nor the workspace impls answer for it".to_owned()
            });
            return stopped(
                steps,
                format!("the chain stops at `{}`: an unproven field", field.name)
            );
        }
    }
    if types.ability_given(type_name, trait_name) {
        return stopped(
            steps,
            format!("`{type_name}` already carries `{trait_name}` — nothing to install")
        );
    }
    steps.push(TraceStep {
        fact:   format!("`{type_name}` does not yet carry `{trait_name}`"),
        reason: "no written impl and no derive in the workspace".to_owned()
    });
    let (spoken, form) = catalog_choice(trait_name, node.fields.len());
    let catalog = CatalogChoice {
        step: steps.len(),
        form
    };
    steps.push(TraceStep {
        fact:   spoken,
        reason: "canonical forms are the catalog read forward".to_owned()
    });
    let Some(gain) = install_impl(sources, settled, file, type_name, trait_name) else {
        return Deduction {
            chain:   Answer {
                refused: false,
                conclusion: "the montage or the re-reading refused the installation".to_owned(),
                steps
            },
            gain:    None,
            catalog: Some(catalog)
        };
    };
    steps.push(TraceStep {
        fact:   format!(
            "the re-reading confirms `{type_name}` now carries `{trait_name}`{}",
            if gain.panics {
                ", with a panic obligation still open"
            } else {
                ", panic-free"
            }
        ),
        reason: "the installed text was read back the way foreign code is read".to_owned()
    });
    Deduction {
        chain:   Answer {
            refused: false,
            conclusion: format!(
                "`{type_name}` gained `{trait_name}` by deduction: every step is a recorded fact"
            ),
            steps
        },
        gain:    Some(gain),
        catalog: Some(catalog)
    }
}

/// The catalog's choice spoken in the vocabulary of the ability it
/// writes — the display forms for `Display`, the field-by-field build
/// for `Default`, a chain must not describe one ability in another's
/// words — paired with the chosen form's catalog name.
fn catalog_choice(trait_name: &str, fields: usize) -> (String, String) {
    if trait_name == "Default" {
        return (
            "the catalog chooses the field-by-field build: every field to its own default"
                .to_owned(),
            "field-by-field".to_owned()
        );
    }
    if fields == 1 {
        (
            "the catalog chooses the bare-value form: one displayed field is the newtype reading"
                .to_owned(),
            "bare-values".to_owned()
        )
    } else {
        (
            "the catalog chooses the labeled-list form, fields in declaration order".to_owned(),
            "labeled-list".to_owned()
        )
    }
}

/// A chain stopped at its failing claim: auditable, and installing
/// nothing.
const fn stopped(steps: Vec<TraceStep>, conclusion: String) -> Deduction {
    Deduction {
        chain:   Answer {
            refused: false,
            conclusion,
            steps
        },
        gain:    None,
        catalog: None
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

    use super::deduce_impl;

    fn one(text: &str) -> Vec<crate::source::Source> {
        vec![crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )]
    }

    /// The full chain: declaration, per-field proofs, absence, the
    /// catalog's choice, and the confirmed re-reading.
    #[test]
    fn a_chain_reaches_a_confirmed_panic_free_gain() {
        let held = deduce_impl(
            &one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"),
            Some(&crate::semantic::settling::compiler_over(&one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"))),
            0,
            "User",
            "Display"
        );
        let gain = held.gain.unwrap();
        assert!(!gain.panics);
        assert_eq!(held.chain.steps.len(), 6, "{:#?}", held.chain.steps);
        assert!(held.chain.steps[1].fact.contains("`name: String`"));
        assert!(held.chain.conclusion.contains("by deduction"));
    }

    /// A `Default` chain speaks its own vocabulary at the catalog step:
    /// the field-by-field build, not a display form.
    #[test]
    fn a_default_chain_speaks_the_default_vocabulary() {
        let held = deduce_impl(
            &one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"),
            Some(&crate::semantic::settling::compiler_over(&one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"))),
            0,
            "User",
            "Default"
        );
        assert!(held.gain.is_some());
        let choice = held
            .chain
            .steps
            .iter()
            .find(|step| step.fact.contains("the catalog chooses"))
            .unwrap();
        assert!(choice.fact.contains("field-by-field build"), "{choice:?}");
        assert!(!choice.fact.contains("labeled-list"), "{choice:?}");
        assert_eq!(
            held.catalog.as_ref().unwrap().form,
            "field-by-field",
            "{:?}",
            held.catalog
        );
    }

    /// The catalog's choice rides the chain as data: the marker names
    /// the very step that states it and the form's catalog name, so an
    /// adviser never parses prose.
    #[test]
    fn the_catalog_marker_names_its_step_and_its_form() {
        let held = deduce_impl(
            &one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"),
            Some(&crate::semantic::settling::compiler_over(&one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"))),
            0,
            "User",
            "Display"
        );
        let marker = held.catalog.unwrap();
        assert_eq!(marker.form, "labeled-list");
        assert!(
            held.chain.steps[marker.step]
                .fact
                .contains("the catalog chooses"),
            "{:?}",
            held.chain.steps[marker.step]
        );
    }

    /// A chain stopped before the choice carries no marker: nothing was
    /// chosen, and the data must not claim otherwise.
    #[test]
    fn a_chain_stopped_before_the_choice_carries_no_marker() {
        let held = deduce_impl(
            &one("pub struct Batch {\n    pub held: Vec<String>,\n}\n"),
            Some(&crate::semantic::settling::compiler_over(&one("pub struct Batch {\n    pub held: Vec<String>,\n}\n"))),
            0,
            "Batch",
            "Display"
        );
        assert!(held.gain.is_none());
        assert!(held.catalog.is_none(), "{:?}", held.catalog);
    }

    /// An unproven field stops the chain at itself, names itself, and
    /// installs nothing.
    #[test]
    fn an_unproven_field_stops_the_chain_at_itself() {
        let held = deduce_impl(
            &one("pub struct Batch {\n    pub held: Vec<String>,\n}\n"),
            Some(&crate::semantic::settling::compiler_over(&one("pub struct Batch {\n    pub held: Vec<String>,\n}\n"))),
            0,
            "Batch",
            "Display"
        );
        assert!(held.gain.is_none());
        assert!(
            held.chain.conclusion.contains("`held`"),
            "{}",
            held.chain.conclusion
        );
        assert!(
            held.chain
                .steps
                .last()
                .unwrap()
                .fact
                .contains("does not provably carry")
        );
    }

}
