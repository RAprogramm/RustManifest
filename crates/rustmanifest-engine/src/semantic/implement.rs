// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The generating direction: an `impl` block written for a type, from
//! the same recorded knowledge the checks read.
//!
//! A workspace trait is implemented from its own written declaration:
//! every required method's signature is sliced verbatim from the source
//! that declares it, so the skeleton compiles against the trait as it
//! is, not as a table remembers it. A well-known std trait comes from a
//! small catalog of canonical forms. Every body is a `todo!()` — the
//! skeleton states the obligations, never invents behavior. Refusals
//! are silence: an unknown type, an unknown trait, a trait already
//! implemented or derived, and a generic trait (whose `impl` header
//! needs arguments no fact records) all answer with nothing.

use super::graph::TypeGraph;
use crate::source::Source;

/// Writes the `impl` block of `trait_name` for `type_name`, or nothing
/// where the facts cannot carry one.
#[must_use]
pub fn impl_skeleton(
    sources: &[Source],
    types: &TypeGraph,
    type_name: &str,
    trait_name: &str
) -> Option<String> {
    types.type_named(type_name)?;
    if types.traits_implemented(type_name).contains(&trait_name) {
        return None;
    }
    let declared = types.traits().iter().find(|node| node.name == trait_name);
    match declared {
        Some(node) => {
            let source = sources.get(node.source)?;
            written_impl(source.text(), type_name, trait_name)
        }
        None => canonical_impl(type_name, trait_name)
    }
}

/// The impl block built from the trait's written declaration: required
/// method signatures sliced verbatim, defaulted methods left to the
/// trait.
fn written_impl(text: &str, type_name: &str, trait_name: &str) -> Option<String> {
    let parsed = syn::parse_file(text).ok()?;
    let declaration = parsed.items.iter().find_map(|item| match item {
        syn::Item::Trait(held) if held.ident == trait_name => Some(held),
        _ => None
    })?;
    if !declaration.generics.params.is_empty() {
        return None;
    }
    let mut lines = vec![format!("impl {trait_name} for {type_name} {{")];
    let mut required = 0usize;
    for item in &declaration.items {
        let syn::TraitItem::Fn(method) = item else {
            continue;
        };
        if method.default.is_some() {
            continue;
        }
        if required > 0 {
            lines.push(String::new());
        }
        required += 1;
        let span = syn::spanned::Spanned::span(&method.sig).byte_range();
        lines.push(format!("    {} {{", text.get(span)?.trim()));
        lines.push("        todo!()".to_owned());
        lines.push("    }".to_owned());
    }
    lines.push("}".to_owned());
    Some(lines.join("\n"))
}

/// The canonical forms of well-known std traits: the catalog the
/// generator reads forward.
fn canonical_impl(type_name: &str, trait_name: &str) -> Option<String> {
    let body = match trait_name {
        "Display" => format!(
            "impl std::fmt::Display for {type_name} {{\n    fn fmt(&self, out: &mut \
             std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        todo!()\n    }}\n}}"
        ),
        "Default" => format!(
            "impl Default for {type_name} {{\n    fn default() -> Self {{\n        todo!()\n    \
             }}\n}}"
        ),
        _ => return None
    };
    Some(body)
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

    use super::impl_skeleton;
    use crate::{semantic::graph::TypeGraph, source::Source};

    fn asked(text: &str, type_name: &str, trait_name: &str) -> Option<String> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let types = TypeGraph::from_sources(&sources);
        impl_skeleton(&sources, &types, type_name, trait_name)
    }

    #[test]
    fn a_workspace_trait_is_implemented_from_its_written_declaration() {
        let written = asked(
            "struct Disk;\ntrait Sink {\n    fn write(&mut self, chunk: &[u8]) -> usize;\n    fn \
             flush(&mut self) {}\n}",
            "Disk",
            "Sink"
        )
        .unwrap();
        assert!(written.starts_with("impl Sink for Disk {"));
        assert!(written.contains("fn write(&mut self, chunk: &[u8]) -> usize {"));
        assert!(written.contains("todo!()"));
        assert!(!written.contains("fn flush"));
    }

    #[test]
    fn an_already_implemented_trait_is_refused() {
        assert!(
            asked(
                "struct Disk;\ntrait Sink { fn write(&mut self); }\nimpl Sink for Disk { fn \
                 write(&mut self) {} }",
                "Disk",
                "Sink"
            )
            .is_none()
        );
    }

    #[test]
    fn a_derived_trait_is_refused() {
        assert!(
            asked(
                "#[derive(Clone)]\nstruct Disk;\ntrait Sink {}",
                "Disk",
                "Clone"
            )
            .is_none()
        );
    }

    #[test]
    fn an_unknown_type_is_refused() {
        assert!(asked("trait Sink { fn write(&mut self); }", "Ghost", "Sink").is_none());
    }

    #[test]
    fn a_generic_trait_is_refused() {
        assert!(
            asked(
                "struct Disk;\ntrait Into2<T> { fn into2(self) -> T; }",
                "Disk",
                "Into2"
            )
            .is_none()
        );
    }

    #[test]
    fn a_known_std_trait_comes_from_the_canon() {
        let written = asked("struct Disk;", "Disk", "Display").unwrap();
        assert!(written.starts_with("impl std::fmt::Display for Disk {"));
        assert!(written.contains("fn fmt(&self, out: &mut std::fmt::Formatter<'_>)"));
        let defaulted = asked("struct Disk;", "Disk", "Default").unwrap();
        assert!(defaulted.contains("fn default() -> Self {"));
    }

    #[test]
    fn an_unknown_trait_is_refused() {
        assert!(asked("struct Disk;", "Disk", "Serialize").is_none());
    }
}
