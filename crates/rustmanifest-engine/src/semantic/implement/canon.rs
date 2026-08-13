// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The canonical forms of well-known std traits, with bodies written
//! from the recorded facts of the type where the facts prove them.
//!
//! A body is real only when every named field carries the needed
//! ability provably, and what proves it is the compiler: the traits it
//! says the field's own type carries, whichever crate declares that
//! type. Two written tables of forty-nine library names answered this
//! before, for the names somebody thought of and for every namesake as
//! well. A single displayed field prints as the bare value — the
//! newtype reading — and several print as a labeled list in declaration
//! order. A default builds the value field by field. Everything the
//! facts cannot prove — enums, tuple and unit structs, a field of an
//! unprovable type — keeps the `todo!()` body: the skeleton then states
//! the obligation without inventing behavior.

use super::super::graph::{TypeGraph, TypeKind, TypeNode};

/// The canonical impl of a well-known std trait for `node`, or nothing
/// where the trait is outside the canon.
pub(super) fn canonical_impl(
    types: &TypeGraph,
    node: &TypeNode,
    trait_name: &str
) -> Option<String> {
    match trait_name {
        "Display" => {
            let body = display_body(types, node).unwrap_or_else(|| "todo!()".to_owned());
            Some(format!(
                "impl std::fmt::Display for {} {{\n    fn fmt(&self, out: &mut \
                 std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        {body}\n    }}\n}}",
                node.name
            ))
        }
        "Default" => {
            let body = default_body(types, node).unwrap_or_else(|| "todo!()".to_owned());
            Some(format!(
                "impl Default for {} {{\n    fn default() -> Self {{\n        {body}\n    \
                 }}\n}}",
                node.name
            ))
        }
        _ => None
    }
}

/// The written display of every named field, where each one provably
/// displays: one field as the bare value, several as a labeled list.
fn display_body(types: &TypeGraph, node: &TypeNode) -> Option<String> {
    let fields = named_fields(node)?;
    if !fields
        .iter()
        .all(|(_, type_name)| types.ability_given(type_name, "Display"))
    {
        return None;
    }
    if let [(name, _)] = fields[..] {
        return Some(format!("write!(out, \"{{}}\", self.{name})"));
    }
    let labels: Vec<String> = fields
        .iter()
        .map(|(name, _)| format!("{name}: {{}}"))
        .collect();
    let values: Vec<String> = fields
        .iter()
        .map(|(name, _)| format!("self.{name}"))
        .collect();
    Some(format!(
        "write!(out, \"{}\", {})",
        labels.join(", "),
        values.join(", ")
    ))
}

/// The value built field by field, where every named field provably
/// carries a default.
fn default_body(types: &TypeGraph, node: &TypeNode) -> Option<String> {
    let fields = named_fields(node)?;
    if !fields
        .iter()
        .all(|(_, type_name)| types.ability_given(type_name, "Default"))
    {
        return None;
    }
    let mut lines = vec!["Self {".to_owned()];
    for (name, _) in &fields {
        lines.push(format!("            {name}: Default::default(),"));
    }
    lines.push("        }".to_owned());
    Some(lines.join("\n"))
}

/// The plainly named fields of a structure, or nothing for enums and
/// for tuple and unit structs, whose members the facts do not name.
fn named_fields(node: &TypeNode) -> Option<Vec<(&str, &str)>> {
    if node.kind != TypeKind::Struct || node.fields.is_empty() {
        return None;
    }
    let fields: Vec<(&str, &str)> = node
        .fields
        .iter()
        .filter(|field| !field.name.contains('.'))
        .map(|field| (field.name.as_str(), field.type_name.as_str()))
        .collect();
    if fields.len() == node.fields.len() {
        Some(fields)
    } else {
        None
    }
}

/// Whether one field type carries `trait_name`, by the same graph the
/// canon bodies are written from — the single proof the deduction chains
/// and the bodies must share, so a chain never claims what a body would
/// refuse.
pub(in crate::semantic) fn field_carries(
    types: &TypeGraph,
    field_type: &str,
    trait_name: &str
) -> bool {
    matches!(trait_name, "Display" | "Default") && types.ability_given(field_type, trait_name)
}
