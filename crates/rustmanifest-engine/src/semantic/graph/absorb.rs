// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Absorption of macro and derive expansion output into the type graph,
//! so generated definitions count as understood.

use rustmanifest_schema::Range;

use super::{
    TypeGraph,
    build::{enum_node, impl_node, node},
    nodes::{Absorption, TypeNode}
};

impl TypeGraph {
    /// Absorbs macro-generated definitions from a crate's expanded stream
    /// (`cargo +nightly rustc -- -Zunpretty=expanded`) into the graph.
    ///
    /// Two kinds of definitions integrate: type definitions absent from
    /// surface syntax (added with `generated: true` and no source anchor),
    /// and impls marked `#[automatically_derived]` for types the graph
    /// knows — surface or just absorbed — each anchored to the type's
    /// source and flagged `derived`. Name and `(type, trait)` duplicates
    /// are skipped, so absorbing the same stream twice is a no-op.
    pub fn absorb_expansion(&mut self, expanded: &str) -> Absorption {
        let Ok(file) = syn::parse_file(expanded) else {
            return Absorption::default();
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        let mut absorbed = Absorption::default();
        for item in &items {
            let generated = match item {
                syn::Item::Struct(definition) => node(usize::MAX, definition),
                syn::Item::Enum(definition) => enum_node(usize::MAX, definition),
                _ => continue
            };
            if self.type_named(&generated.name).is_some() {
                continue;
            }
            self.types.push(TypeNode {
                range: Range {
                    start: 0, end: 0
                },
                generated: true,
                ..generated
            });
            absorbed.types += 1;
        }
        for item in items {
            let syn::Item::Impl(definition) = item else {
                continue;
            };
            let automatically_derived = definition
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("automatically_derived"));
            if !automatically_derived {
                continue;
            }
            let Some(mut node) = impl_node(0, definition) else {
                continue;
            };
            let Some(subject) = self.type_named(&node.type_name) else {
                continue;
            };
            let duplicate = self.impls.iter().any(|existing| {
                existing.type_name == node.type_name && existing.trait_name == node.trait_name
            });
            if duplicate {
                continue;
            }
            node.source = subject.source;
            node.derived = true;
            self.impls.push(node);
            absorbed.impls += 1;
        }
        absorbed
    }
}
