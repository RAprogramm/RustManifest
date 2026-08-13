// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The canonical forms of well-known std traits, with bodies written
//! from the recorded facts of the type where the facts prove them.
//!
//! A body is real only when every named field carries the needed
//! ability provably: the std tables answer for well-known field types,
//! the type graph for workspace field types by their written impls and
//! derives. A single displayed field prints as the bare value — the
//! newtype reading — and several print as a labeled list in declaration
//! order. A default builds the value field by field. Everything the
//! facts cannot prove — enums, tuple and unit structs, a field of an
//! unprovable type — keeps the `todo!()` body: the skeleton then states
//! the obligation without inventing behavior.

use super::super::graph::{TypeGraph, TypeKind, TypeNode};

/// Well-known std field types that implement `Display`.
const DISPLAYED: [&str; 22] = [
    "IpAddr",
    "Ipv4Addr",
    "Ipv6Addr",
    "SocketAddr",
    "String",
    "bool",
    "char",
    "f32",
    "f64",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "str",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize"
];

/// Well-known std field types that implement `Default` for any type
/// argument they carry.
const DEFAULTED: [&str; 27] = [
    "BTreeMap", "BTreeSet", "Duration", "HashMap", "HashSet", "Option", "OsString", "PathBuf",
    "String", "Vec", "VecDeque", "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128",
    "isize", "u8", "u16", "u32", "u64", "u128", "usize"
];

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
        .all(|(_, type_name)| proves(types, type_name, "Display", &DISPLAYED))
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
        .all(|(_, type_name)| proves(types, type_name, "Default", &DEFAULTED))
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

/// Whether one field type provably carries `trait_name`, by the same
/// tables and graph the canon bodies are written from — the single
/// proof the deduction chains and the bodies must share, so a chain
/// never claims what a body would refuse.
pub(in crate::semantic) fn field_carries(
    types: &TypeGraph,
    field_type: &str,
    trait_name: &str
) -> bool {
    match trait_name {
        "Display" => proves(types, field_type, "Display", &DISPLAYED),
        "Default" => proves(types, field_type, "Default", &DEFAULTED),
        _ => false
    }
}

/// Whether `type_name` provably carries `trait_name`: the std table
/// answers for well-known types, the type graph for workspace types by
/// what the compiler says the type carries.
fn proves(types: &TypeGraph, type_name: &str, trait_name: &str, table: &[&str]) -> bool {
    table.contains(&type_name) || types.ability_given(type_name, trait_name)
}
