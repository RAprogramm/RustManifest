// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The form families of the advisory precedent: which written bodies
//! state a precedent, and which catalog forms each family may rank.
//!
//! A family pairs a trait's catalog forms with a reading of the bodies
//! that already chose one. The reading claims a form only where the
//! written body proves it — a `Display` template labels its holes or
//! prints bare values; a `Default` body either builds the owner field
//! by field through a written struct literal or delegates the value to
//! a call. A body proving neither states no precedent: silence, not a
//! guess. A precedent pool advises only its own family's forms, so a
//! printing precedent never speaks on a `Default` chain.

use rustmanifest_engine::{TypeNode, Workspace};

use crate::FormPrecedent;

/// The stable feature names of one type instance, for form ranking.
pub const FORM_FEATURES: [&str; 2] = ["multi_field", "has_string_field"];

/// The display forms a printing precedent ranks.
pub const DISPLAY_FORMS: [&str; 2] = ["labeled-list", "bare-values"];

/// The default forms a written `default` body can prove: the owner
/// built field by field, or the value delegated to a call.
pub const DEFAULT_FORMS: [&str; 2] = ["field-by-field", "delegating"];

/// The binary features of one type node, in [`FORM_FEATURES`] order.
#[must_use]
pub fn type_form_features(node: &TypeNode) -> Vec<u8> {
    vec![
        u8::from(node.fields.len() > 1),
        u8::from(node.fields.iter().any(|field| field.type_name == "String")),
    ]
}

/// The precedent pool whose family holds the chosen form, with the
/// forms that pool has standing to rank — or nothing, for a form no
/// family claims.
#[must_use]
pub fn precedents_for(
    workspace: &Workspace<'_>,
    form: &str
) -> Option<(Vec<FormPrecedent>, &'static [&'static str])> {
    if DISPLAY_FORMS.contains(&form) {
        return Some((display_precedents(workspace), &DISPLAY_FORMS));
    }
    if DEFAULT_FORMS.contains(&form) {
        return Some((default_precedents(workspace), &DEFAULT_FORMS));
    }
    None
}

/// The display-form precedents the workspace's own written `Display`
/// impls state: the features of each implementing type, and whether
/// its written template labels its holes or prints bare values.
fn display_precedents(workspace: &Workspace<'_>) -> Vec<FormPrecedent> {
    use rustmanifest_engine::semantic::{Payload, Segment};
    let types = workspace.types();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        let Some(owner) = fact.function.strip_suffix("::fmt") else {
            continue;
        };
        let Some(node) = types.type_named(owner) else {
            continue;
        };
        if !types.traits_implemented(owner).contains(&"Display") {
            continue;
        }
        let labeled = fact.output_writes.iter().any(|write| {
            matches!(&write.payload, Payload::Template(segments)
            if segments.iter().any(|segment| {
                matches!(segment, Segment::Text(text) if text.contains(':'))
            }))
        });
        found.push(FormPrecedent {
            features: type_form_features(node),
            chose:    if labeled {
                "labeled-list".to_owned()
            } else {
                "bare-values".to_owned()
            }
        });
    }
    found
}

/// The default-form precedents the workspace's own written `Default`
/// impls state: a body whose struct literal constructs the owner chose
/// the field-by-field build; a body constructing no owner but writing
/// a call chose delegation; a body proving neither states nothing.
///
/// A literal spelled `Self` counts as the owner: inside the owner's own
/// `default` the language defines `Self` as the owner, while the fact
/// channel resolves the spelling only through a receiver — and an
/// associated function has none — so the reading accepts both
/// spellings instead of asking the channel for a new record.
fn default_precedents(workspace: &Workspace<'_>) -> Vec<FormPrecedent> {
    let types = workspace.types();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        let Some(owner) = fact.function.strip_suffix("::default") else {
            continue;
        };
        let Some(node) = types.type_named(owner) else {
            continue;
        };
        if !types.traits_implemented(owner).contains(&"Default") {
            continue;
        }
        let built = fact
            .constructs
            .iter()
            .any(|name| name == owner || name == "Self");
        if !built && fact.calls.is_empty() {
            continue;
        }
        found.push(FormPrecedent {
            features: type_form_features(node),
            chose:    if built {
                "field-by-field".to_owned()
            } else {
                "delegating".to_owned()
            }
        });
    }
    found
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

    use rustmanifest_engine::{Source, Workspace};

    use super::{default_precedents, precedents_for};

    fn one(text: &str) -> [Source; 1] {
        [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    /// A written struct literal of the owner proves the field-by-field
    /// form.
    #[test]
    fn an_owner_literal_proves_the_field_by_field_form() {
        let sources = one(
            "pub struct Tag { pub name: String }\nimpl Default for Tag { fn default() -> Self { \
             Self { name: String::new() } } }"
        );
        let found = default_precedents(&Workspace::new(&sources));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].chose, "field-by-field");
    }

    /// A body constructing no owner but writing a call proves
    /// delegation.
    #[test]
    fn a_call_without_an_owner_literal_proves_delegation() {
        let sources = one(
            "pub struct Tag { pub name: String }\nimpl Default for Tag { fn default() -> Self { \
             Self::fresh() } }"
        );
        let found = default_precedents(&Workspace::new(&sources));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].chose, "delegating");
    }

    /// A body proving neither form states no precedent: silence, not a
    /// guess.
    #[test]
    fn a_body_proving_neither_form_states_nothing() {
        let sources = one(
            "pub struct Tag { pub name: String }\nimpl Default for Tag { fn default() -> Self { \
             loop {} } }"
        );
        assert!(default_precedents(&Workspace::new(&sources)).is_empty());
    }

    /// A form no family claims gets no pool: nobody has standing.
    #[test]
    fn a_form_outside_every_family_gets_no_pool() {
        let sources = one("pub struct Tag { pub name: String }");
        assert!(precedents_for(&Workspace::new(&sources), "unheard-of").is_none());
    }
}
