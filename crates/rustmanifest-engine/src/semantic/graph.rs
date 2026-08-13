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
    types:   Vec<TypeNode>,
    impls:   Vec<ImplNode>,
    traits:  Vec<TraitNode>,
    /// What the compiler says each type carries, by type and trait name.
    granted: std::collections::BTreeSet<(String, String)>
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

    /// Whether `name` already carries `trait_name`.
    ///
    /// The compiler's answer, which covers a written `impl` and a derive
    /// alike: a reading of the source has to know what every derive
    /// expands to, and the one that generates an ability from its own
    /// attributes is the case a bare-name install used to collide with.
    #[must_use]
    pub fn ability_given(&self, name: &str, trait_name: &str) -> bool {
        self.granted
            .iter()
            .any(|(held, carried)| held == name && carried == trait_name)
    }

    /// States what the compiler settled about the workspace: which traits
    /// each type carries, and what each declared type is made of.
    pub fn granted_by(&mut self, settled: Option<&rustmanifest_schema::Resolution>) {
        let Some(resolution) = settled else {
            return;
        };
        for declared in &resolution.declared {
            let Some(node) = self
                .types
                .iter_mut()
                .find(|node| node.name == declared.named)
            else {
                continue;
            };
            for settled in &declared.fields {
                if let Some(field) = node
                    .fields
                    .iter_mut()
                    .find(|held| held.name == settled.name)
                {
                    let opened = opened(&settled.named);
                    if let Some(named) = crate::semantic::behavior::outermost_name(&settled.named) {
                        field.type_name = named;
                    }
                    field.holds =
                        crate::semantic::behavior::outermost_name(&opened).filter(|named| {
                            *named != field.type_name
                        });
                    field.element = held(&opened);
                    field.key = looked_up_by(&opened);
                }
            }
        }
        self.granted = resolution
            .grants
            .iter()
            .filter_map(|row| {
                Some((
                    crate::semantic::behavior::outermost_name(&row.named)?,
                    row.trait_name.clone()
                ))
            })
            .collect();
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

    #[test]
    fn interaction_edges_skip_self_and_external_references() {
        let graph = graph(&[
            "struct Game;\nimpl Game { fn tick(&self) { Game::helper(); String::new(); } fn \
             helper() {} }"
        ]);
        assert!(graph.interaction_edges().is_empty());
    }
}

/// The type standing under every single-argument wrapper of a rendered
/// type.
///
/// A value behind a lock, a box or a reference-counted handle is that
/// value: the wrapper says how it is held, not what it is. Peeling by
/// shape rather than by a list of wrapper names answers for a wrapper
/// this workspace wrote itself.
fn opened(rendered: &str) -> String {
    let mut standing = rendered.trim().trim_start_matches('&').trim().to_owned();
    loop {
        let Some(inside) = single_argument(&standing) else {
            return standing;
        };
        standing = inside;
    }
}

/// What a rendered collection holds: its last type argument, so a map
/// answers with its values and a sequence with its elements.
fn held(rendered: &str) -> Option<String> {
    let inside = rendered.split_once('<')?.1.strip_suffix('>')?;
    let last = arguments(inside).pop()?;
    crate::semantic::behavior::outermost_name(&last)
}

/// The single type argument of a rendered type, where it has exactly one.
fn single_argument(rendered: &str) -> Option<String> {
    let inside = rendered.split_once('<')?.1.strip_suffix('>')?;
    let mut written = arguments(inside);
    (written.len() == 1).then(|| written.pop()).flatten()
}

/// The top-level arguments of a rendered argument list.
fn arguments(inside: &str) -> Vec<String> {
    let mut written = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (at, letter) in inside.char_indices() {
        match letter {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                written.push(inside[start..at].trim().to_owned());
                start = at + 1;
            }
            _ => {}
        }
    }
    written.push(inside[start..].trim().to_owned());
    written
}

/// What a rendered map is looked into with: its first type argument,
/// where it carries two.
fn looked_up_by(rendered: &str) -> Option<String> {
    let inside = rendered.split_once('<')?.1.strip_suffix('>')?;
    let written = arguments(inside);
    let [key, _] = written.as_slice() else {
        return None;
    };
    crate::semantic::behavior::outermost_name(key)
}
