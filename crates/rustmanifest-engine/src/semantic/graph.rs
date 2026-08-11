// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace type graph: structures, enums, traits, impl blocks, and the
//! edges between them.
//!
//! The graph records what each type is made of (aggregation: "`User`
//! aggregates `Credentials` through field `credentials`") and how types
//! act on each other (interaction: a method body of one type's impl calls
//! an associated function of another workspace type), giving semantic
//! checks the material for relationship-level reasoning.

mod absorb;
mod build;
mod nodes;

use std::collections::BTreeSet;

use self::build::{enum_node, impl_node, node, trait_node};
pub use self::nodes::{
    Absorption, FieldNode, ImplNode, MethodNode, TraitNode, TypeKind, TypeNode
};
use crate::source::Source;

/// All type, trait, and impl definitions of the analyzed workspace slice.
#[derive(Debug, Default)]
pub struct TypeGraph {
    types:  Vec<TypeNode>,
    impls:  Vec<ImplNode>,
    traits: Vec<TraitNode>
}

impl TypeGraph {
    /// Builds the graph by parsing every source; files that do not parse as
    /// Rust are skipped.
    #[must_use]
    pub fn from_sources(sources: &[Source]) -> Self {
        let mut graph = Self::default();
        for (index, source) in sources.iter().enumerate() {
            let Some(file) = source.tree() else {
                continue;
            };
            graph.collect(index, &file.items, false);
        }
        graph
    }

    /// Collects the items of one nesting level, descending into inline
    /// modules with the depth recorded: a type declared inside one is a
    /// different installation target than a top-level declaration, and
    /// the node must say so.
    fn collect(&mut self, index: usize, items: &[syn::Item], nested: bool) {
        for item in items {
            match item {
                syn::Item::Struct(definition) => {
                    self.types.push(node(index, definition, nested));
                }
                syn::Item::Enum(definition) => {
                    self.types.push(enum_node(index, definition, nested));
                }
                syn::Item::Impl(definition) => {
                    if let Some(impl_node) = impl_node(index, definition) {
                        self.impls.push(impl_node);
                    }
                }
                syn::Item::Trait(definition) => {
                    self.traits.push(trait_node(index, definition));
                }
                syn::Item::Mod(module) => {
                    if let Some((_, inner)) = &module.content {
                        self.collect(index, inner, true);
                    }
                }
                _ => {}
            }
        }
    }

    /// Structures and enums in deterministic discovery order.
    #[must_use]
    pub fn types(&self) -> &[TypeNode] {
        &self.types
    }

    /// Impl blocks in deterministic discovery order.
    #[must_use]
    pub fn impls(&self) -> &[ImplNode] {
        &self.impls
    }

    /// Trait definitions in deterministic discovery order.
    #[must_use]
    pub fn traits(&self) -> &[TraitNode] {
        &self.traits
    }

    /// Returns the structure named `name`, if the workspace defines it.
    #[must_use]
    pub fn type_named(&self, name: &str) -> Option<&TypeNode> {
        self.types.iter().find(|node| node.name == name)
    }

    /// Returns the trait names `name` implements, surface and derived,
    /// deduplicated and sorted.
    #[must_use]
    pub fn traits_implemented(&self, name: &str) -> Vec<&str> {
        let mut traits: Vec<&str> = self
            .impls
            .iter()
            .filter(|impl_block| impl_block.type_name == name)
            .filter_map(|impl_block| impl_block.trait_name.as_deref())
            .collect();
        traits.sort_unstable();
        traits.dedup();
        traits
    }

    /// Whether `name` provably carries `trait_name` with no new impl: a
    /// written impl, a matching derive, or a derive whose contract
    /// writes the ability itself — the error-deriving crate's `Error`
    /// derive generates the `Display` impl from its own attributes, so
    /// a bare-name install would collide with the expansion the graph
    /// cannot see. The corpus judge's third catch.
    #[must_use]
    pub fn ability_given(&self, name: &str, trait_name: &str) -> bool {
        const DERIVE_GRANTS: [(&str, &str); 1] = [("Error", "Display")];
        if self.traits_implemented(name).contains(&trait_name) {
            return true;
        }
        let Some(node) = self.type_named(name) else {
            return false;
        };
        node.derives.iter().any(|derive| {
            derive == trait_name
                || DERIVE_GRANTS
                    .iter()
                    .any(|(granting, granted)| derive == granting && *granted == trait_name)
        })
    }

    /// Returns every structure that aggregates `name` through a field whose
    /// type resolves to it.
    #[must_use]
    pub fn aggregators_of(&self, name: &str) -> Vec<&TypeNode> {
        self.types
            .iter()
            .filter(|node| node.field_of_type(name).is_some())
            .collect()
    }

    /// Returns every resolved aggregation edge of the workspace: the owning
    /// structure, the aggregating field, and the workspace structure the
    /// field's type name resolves to.
    ///
    /// Fields whose type name does not match a workspace structure produce
    /// no edge. Edges follow the deterministic discovery order of
    /// [`Self::types`] and field declaration order within each structure.
    #[must_use]
    pub fn aggregation_edges(&self) -> Vec<(&TypeNode, &FieldNode, &TypeNode)> {
        let mut edges = Vec::new();
        for owner in &self.types {
            for field in &owner.fields {
                if let Some(target) = self.type_named(&field.type_name) {
                    edges.push((owner, field, target));
                }
            }
        }
        edges
    }

    /// Returns every resolved interaction edge of the workspace: one type
    /// acting on another through a qualified path in a method body of its
    /// impl blocks (`Board::place` inside `impl Game` yields
    /// `Game -> Board`).
    ///
    /// Only references that resolve to a workspace type produce an edge;
    /// self-references are skipped. Edges are deduplicated and follow impl
    /// discovery order.
    #[must_use]
    pub fn interaction_edges(&self) -> Vec<(&ImplNode, &TypeNode)> {
        let mut edges = Vec::new();
        let mut seen = BTreeSet::new();
        for impl_block in &self.impls {
            for method in &impl_block.methods {
                for reference in &method.type_refs {
                    if reference == &impl_block.type_name {
                        continue;
                    }
                    let Some(target) = self.type_named(reference) else {
                        continue;
                    };
                    if seen.insert((impl_block.type_name.clone(), target.name.clone())) {
                        edges.push((impl_block, target));
                    }
                }
            }
        }
        edges
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

    use super::TypeGraph;
    use crate::source::Source;

    /// A declaration inside an inline module carries the nesting flag;
    /// a top-level one does not — the installation target differs, and
    /// the node says so.
    #[test]
    fn a_nested_declaration_carries_the_nesting_flag() {
        let held = graph(&[
            "pub struct Open { pub name: String }\nmod inner { pub struct Hidden \
                            { pub name: String } }"
        ]);
        assert!(!held.type_named("Open").unwrap().nested);
        assert!(held.type_named("Hidden").unwrap().nested);
    }

    fn graph(texts: &[&str]) -> TypeGraph {
        let sources: Vec<Source> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| {
                Source::new(
                    PathBuf::from(format!("src/file{index}.rs")),
                    (*text).to_owned()
                )
            })
            .collect();
        TypeGraph::from_sources(&sources)
    }

    #[test]
    fn structures_and_fields_are_recorded() {
        let graph = graph(&["struct User { identity: Identity, age: u8 }"]);
        let user = graph.type_named("User").unwrap();
        assert_eq!(user.fields.len(), 2);
        assert_eq!(user.fields[0].type_name, "Identity");
    }

    #[test]
    fn aggregation_is_resolvable_across_files() {
        let graph = graph(&[
            "struct Identity { id: u64 }",
            "struct User { identity: Identity }"
        ]);
        let aggregators = graph.aggregators_of("Identity");
        assert_eq!(aggregators.len(), 1);
        assert_eq!(aggregators[0].name, "User");
        assert_eq!(
            aggregators[0].field_of_type("Identity").unwrap().name,
            "identity"
        );
    }

    #[test]
    fn references_resolve_to_their_target_type() {
        let graph = graph(&[
            "struct Identity { id: u64 }",
            "struct View<'a> { identity: &'a Identity }"
        ]);
        assert_eq!(graph.aggregators_of("Identity").len(), 1);
    }

    #[test]
    fn unparsable_files_are_skipped() {
        let graph = graph(&["struct Ok { a: u8 }", "fn broken("]);
        assert_eq!(graph.types().len(), 1);
    }

    #[test]
    fn aggregation_edges_resolve_across_files() {
        let graph = graph(&[
            "struct Identity { id: u64 }",
            "struct User { identity: Identity, age: u8 }"
        ]);
        let edges = graph.aggregation_edges();
        assert_eq!(edges.len(), 1);
        let (owner, field, target) = edges[0];
        assert_eq!(owner.name, "User");
        assert_eq!(field.name, "identity");
        assert_eq!(target.name, "Identity");
    }

    #[test]
    fn aggregation_edges_are_empty_without_workspace_targets() {
        let graph = graph(&["struct User { id: u64, name: String }"]);
        assert!(graph.aggregation_edges().is_empty());
    }

    #[test]
    fn nested_module_structs_are_recorded() {
        let graph = graph(&["mod inner { pub struct Deep { a: u8 } }"]);
        assert!(graph.type_named("Deep").is_some());
    }

    #[test]
    fn enums_are_recorded_with_variant_payloads() {
        let graph = graph(&[
            "struct Move { x: u8 }",
            "enum Event { Moved(Move), Named { actor: String }, Quit }"
        ]);
        let event = graph.type_named("Event").unwrap();
        assert_eq!(event.kind, super::TypeKind::Enum);
        assert_eq!(event.fields.len(), 2);
        assert_eq!(event.fields[0].name, "Moved");
        assert_eq!(event.fields[0].type_name, "Move");
        assert_eq!(event.fields[1].name, "Named.actor");
        assert_eq!(graph.aggregation_edges().len(), 1);
    }

    #[test]
    fn traits_and_impls_are_recorded() {
        let graph = graph(&[
            "trait Render { fn draw(&self); fn clear(&self); }\nstruct Board { cells: u8 \
             }\nimpl Render for Board { fn draw(&self) {} fn clear(&self) {} }\nimpl Board { \
             fn size(&self) -> u8 { self.cells } }"
        ]);
        assert_eq!(graph.traits().len(), 1);
        assert_eq!(graph.traits()[0].methods, vec!["draw", "clear"]);
        assert_eq!(graph.impls().len(), 2);
        assert_eq!(graph.impls()[0].trait_name.as_deref(), Some("Render"));
        assert_eq!(graph.impls()[1].trait_name, None);
        assert_eq!(graph.impls()[1].methods[0].name, "size");
    }

    #[test]
    fn interaction_edges_link_caller_to_callee_type() {
        let graph = graph(&[
            "struct Board;\nimpl Board { pub fn reset() {} }",
            "struct Game;\nimpl Game { fn restart(&self) { Board::reset(); } }"
        ]);
        let edges = graph.interaction_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0.type_name, "Game");
        assert_eq!(edges[0].1.name, "Board");
    }

    const EXPANDED: &str = "#[automatically_derived]\nimpl ::core::fmt::Debug for User { \
                            }\n#[automatically_derived]\nimpl ::core::clone::Clone for User { \
                            }\n#[automatically_derived]\nimpl ::core::fmt::Debug for Ghost { }";

    #[test]
    fn absorbed_expansion_adds_derived_impls_for_known_types() {
        let mut graph = graph(&["struct User { id: u64 }"]);
        assert_eq!(graph.absorb_expansion(EXPANDED).impls, 2);
        assert_eq!(graph.traits_implemented("User"), vec!["Clone", "Debug"]);
        let derived = graph.impls().iter().find(|node| node.derived).unwrap();
        assert_eq!(derived.source, graph.type_named("User").unwrap().source);
    }

    #[test]
    fn absorbing_the_same_stream_twice_is_a_no_op() {
        let mut graph = graph(&["struct User { id: u64 }"]);
        graph.absorb_expansion(EXPANDED);
        assert_eq!(
            graph.absorb_expansion(EXPANDED),
            super::Absorption::default()
        );
        assert_eq!(graph.impls().len(), 2);
    }

    #[test]
    fn impls_for_unknown_types_and_broken_streams_absorb_nothing() {
        let mut graph = graph(&["struct Other;"]);
        assert_eq!(graph.absorb_expansion(EXPANDED).impls, 0);
        assert_eq!(
            graph.absorb_expansion("fn broken("),
            super::Absorption::default()
        );
    }

    #[test]
    fn generated_types_absorb_with_their_derived_impls() {
        let mut graph = graph(&["struct Surface { id: u64 }"]);
        let absorbed = graph.absorb_expansion(
            "struct Surface { id: u64 }\nstruct CreateUser { email: String \
             }\n#[automatically_derived]\nimpl ::core::fmt::Debug for CreateUser { }"
        );
        assert_eq!(absorbed.types, 1);
        assert_eq!(absorbed.impls, 1);
        let generated = graph.type_named("CreateUser").unwrap();
        assert!(generated.generated);
        assert_eq!(generated.fields[0].name, "email");
        assert_eq!(graph.traits_implemented("CreateUser"), vec!["Debug"]);
        assert!(!graph.type_named("Surface").unwrap().generated);
    }

    #[test]
    fn surface_and_derived_traits_merge_in_traits_implemented() {
        let mut graph = graph(&["struct User { id: u64 }\nimpl core::fmt::Display for User { }"]);
        graph.absorb_expansion(EXPANDED);
        assert_eq!(
            graph.traits_implemented("User"),
            vec!["Clone", "Debug", "Display"]
        );
    }

    #[test]
    fn interaction_edges_skip_self_and_external_references() {
        let graph = graph(&[
            "struct Game;\nimpl Game { fn tick(&self) { Game::helper(); String::new(); } fn \
             helper() {} }"
        ]);
        assert!(graph.interaction_edges().is_empty());
    }
}
