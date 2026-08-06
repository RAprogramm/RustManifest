// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The names this tier reads off syntax: a path rendered against the
//! enclosing file's imports, the table key of a field member, and the
//! name a single binding pattern introduces.

use std::collections::BTreeMap;

/// Renders a path as segment names, the first segment substituted with
/// its full `use` path when the file imported it.
pub(super) fn expanded_segments(
    imports: &BTreeMap<String, String>,
    path: &syn::Path
) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    for (position, segment) in path.segments.iter().enumerate() {
        let name = segment.ident.to_string();
        if position == 0 {
            match imports.get(&name) {
                Some(full) => segments.extend(full.split("::").map(str::to_owned)),
                None => segments.push(name)
            }
        } else {
            segments.push(name);
        }
    }
    segments
}

/// Table key of a field member: the written name, or the position for
/// the unnamed fields of a tuple struct — the shape newtype wrappers
/// carry their payload in. Used by the typing channel only; the
/// read/write fact channel keeps naming its members as written.
pub(super) fn member_name(member: &syn::Member) -> String {
    match member {
        syn::Member::Named(name) => name.to_string(),
        syn::Member::Unnamed(index) => index.index.to_string()
    }
}

/// Name of a single binding pattern, seen through references and type
/// ascriptions.
pub(super) fn binding_name(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Reference(reference) => binding_name(&reference.pat),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        syn::Pat::Paren(inner) => binding_name(&inner.pat),
        syn::Pat::Guard(guarded) => binding_name(&guarded.pat),
        _ => None
    }
}
