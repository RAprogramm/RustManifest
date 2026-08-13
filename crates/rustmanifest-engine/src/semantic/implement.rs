// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The generating direction: an `impl` block written for a type, from
//! the same recorded knowledge the checks read.
//!
//! A workspace trait is implemented from its own written declaration:
//! every required method's signature is sliced verbatim from the source
//! that declares it, so the skeleton compiles against the trait as it
//! is, not as a table remembers it. A well-known std trait comes from
//! the canon, with a body written from the type's recorded fields
//! where the facts prove one — and a `todo!()` where they cannot: the
//! skeleton then states the obligation without inventing behavior.
//! Refusals are silence: an unknown type, an unknown trait, a trait
//! already implemented or derived, a generic trait or a generic type
//! (whose `impl` header needs arguments no fact records), and a type
//! declared inside an inline module all answer with nothing. The last
//! two refusals are the corpus judge's first catches: the montage
//! lands text at the file's top level, where a nested declaration is
//! out of scope and a bare-name header leaves a generic unnamed — and
//! the re-reading, matching the bare name, would confirm the lie the
//! compiler then refuses.

pub(super) mod canon;

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
    let asked = types.type_named(type_name)?;
    if asked.nested || asked.generic {
        return None;
    }
    if types.ability_given(type_name, trait_name) {
        return None;
    }
    let declared = types.traits().iter().find(|node| node.name == trait_name);
    match declared {
        Some(node) => {
            let source = sources.get(node.source)?;
            written_impl(source.text(), type_name, trait_name)
        }
        None => canon::canonical_impl(types, asked, trait_name)
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
        let mut types = TypeGraph::from_sources(&sources);
        types.granted_by(
            &sources,
            Some(&crate::semantic::settling::settled_over(&sources))
        );
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

    /// A type declared inside an inline module is refused: the montage
    /// lands text at the file's top level, where the declaration is out
    /// of scope — the corpus judge's first catch.
    #[test]
    fn a_type_declared_inside_an_inline_module_is_refused() {
        assert!(
            asked(
                "mod held {\n    pub struct Hidden {\n        pub name: String,\n    }\n}\n",
                "Hidden",
                "Display"
            )
            .is_none()
        );
    }

    /// A generic type is refused: a bare-name `impl` header names no
    /// parameter the declaration demands — the corpus judge's second
    /// catch.
    #[test]
    fn a_generic_type_is_refused() {
        assert!(
            asked(
                "pub enum Held<T> {\n    Kept(T),\n    Empty,\n}\n",
                "Held",
                "Default"
            )
            .is_none()
        );
    }

    /// A derive whose contract writes the ability itself refuses the
    /// install: the error-deriving crate's `Error` derive generates
    /// `Display` from its own attributes, and a bare-name impl would
    /// collide with that expansion — the corpus judge's third catch.
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

    #[test]
    fn a_display_body_is_written_from_provably_displayed_fields() {
        let written = asked(
            "struct User { id: String, name: String, age: u8 }",
            "User",
            "Display"
        )
        .unwrap();
        assert!(
            written.contains(
                "write!(out, \"id: {}, name: {}, age: {}\", self.id, self.name, self.age)"
            ),
            "{written}"
        );
        assert!(!written.contains("todo!()"), "{written}");
    }

    #[test]
    fn a_single_displayed_field_prints_as_the_bare_value() {
        let written = asked("struct Tag { name: String }", "Tag", "Display").unwrap();
        assert!(
            written.contains("write!(out, \"{}\", self.name)"),
            "{written}"
        );
    }

    #[test]
    fn an_unprovable_field_keeps_the_display_body_a_stub() {
        let written = asked("struct Batch { entries: Vec<String> }", "Batch", "Display").unwrap();
        assert!(written.contains("todo!()"), "{written}");
    }

    #[test]
    fn a_default_body_builds_the_value_field_by_field() {
        let written = asked(
            "struct User { id: String, name: String, age: u8 }",
            "User",
            "Default"
        )
        .unwrap();
        assert!(written.contains("Self {"), "{written}");
        assert!(written.contains("id: Default::default(),"), "{written}");
        assert!(written.contains("age: Default::default(),"), "{written}");
        assert!(!written.contains("todo!()"), "{written}");
    }

    #[test]
    fn a_field_without_a_provable_default_keeps_the_stub() {
        let written = asked("struct Held { moment: Instant }", "Held", "Default").unwrap();
        assert!(written.contains("todo!()"), "{written}");
    }

    #[test]
    fn an_enum_keeps_the_stub_body() {
        let written = asked("enum State { On, Off }", "State", "Display").unwrap();
        assert!(written.contains("todo!()"), "{written}");
    }
}
