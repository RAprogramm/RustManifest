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

use std::collections::BTreeSet;

use rustmanifest_schema::Range;
use syn::visit::Visit;

use crate::source::Source;

/// Kind of a definition recorded as a [`TypeNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    /// A `struct` definition.
    Struct,
    /// An `enum` definition; its variant payloads become the node's fields.
    Enum
}

/// One named field of a structure, or one variant payload of an enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldNode {
    /// Field name as written; for enum payloads `variant.field` or the
    /// variant name for unnamed payloads.
    pub name:      String,
    /// Last path segment of the field's type (`Vec<Credentials>` → `Vec`).
    pub type_name: String
}

/// One structure or enum definition found in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeNode {
    /// Type name.
    pub name:   String,
    /// Index of the source the type was parsed from.
    pub source: usize,
    /// Byte range of the type's identifier.
    pub range:  Range,
    /// Whether the node is a structure or an enum.
    pub kind:   TypeKind,
    /// Named fields (or enum variant payloads) in declaration order; empty
    /// for tuple and unit structs.
    pub fields: Vec<FieldNode>
}

/// One method of an impl block, with the workspace-type references its
/// body makes through qualified paths (`Board::new`, `Rules::DEFAULT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodNode {
    /// Method name as written.
    pub name:      String,
    /// First segments of qualified paths in the body that look like type
    /// names, deduplicated and sorted.
    pub type_refs: Vec<String>
}

/// One `impl` block found in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplNode {
    /// Last path segment of the self type (`impl Foo for Bar` → `Bar`).
    pub type_name:  String,
    /// Last path segment of the implemented trait, if any.
    pub trait_name: Option<String>,
    /// Index of the source the impl was parsed from.
    pub source:     usize,
    /// Methods and associated functions in declaration order.
    pub methods:    Vec<MethodNode>
}

/// One trait definition found in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNode {
    /// Trait name.
    pub name:    String,
    /// Index of the source the trait was parsed from.
    pub source:  usize,
    /// Declared method names in declaration order.
    pub methods: Vec<String>
}

impl TypeNode {
    /// Returns the field that aggregates `type_name`, if any.
    #[must_use]
    pub fn field_of_type(&self, type_name: &str) -> Option<&FieldNode> {
        self.fields
            .iter()
            .find(|field| field.type_name == type_name)
    }
}

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
            let Ok(file) = syn::parse_file(source.text()) else {
                continue;
            };
            let mut items = Vec::new();
            crate::ast::flattened(&file.items, &mut items);
            for item in items {
                match item {
                    syn::Item::Struct(definition) => graph.types.push(node(index, definition)),
                    syn::Item::Enum(definition) => graph.types.push(enum_node(index, definition)),
                    syn::Item::Impl(definition) => {
                        if let Some(impl_node) = impl_node(index, definition) {
                            graph.impls.push(impl_node);
                        }
                    }
                    syn::Item::Trait(definition) => {
                        graph.traits.push(trait_node(index, definition));
                    }
                    _ => {}
                }
            }
        }
        graph
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

/// Builds a [`TypeNode`] from a parsed struct definition.
fn node(source: usize, definition: &syn::ItemStruct) -> TypeNode {
    let bytes = definition.ident.span().byte_range();
    let fields = match &definition.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| {
                field.ident.as_ref().map(|ident| FieldNode {
                    name:      ident.to_string(),
                    type_name: last_segment(&field.ty)
                })
            })
            .collect(),
        _ => Vec::new()
    };
    TypeNode {
        name: definition.ident.to_string(),
        source,
        range: byte_range(&bytes),
        kind: TypeKind::Struct,
        fields
    }
}

/// Converts a span byte range into the schema [`Range`].
fn byte_range(bytes: &std::ops::Range<usize>) -> Range {
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}

/// Builds a [`TypeNode`] from a parsed enum definition; every variant
/// payload becomes one field entry.
fn enum_node(source: usize, definition: &syn::ItemEnum) -> TypeNode {
    let bytes = definition.ident.span().byte_range();
    let mut fields = Vec::new();
    for variant in &definition.variants {
        for field in &variant.fields {
            let name = field.ident.as_ref().map_or_else(
                || variant.ident.to_string(),
                |ident| format!("{}.{ident}", variant.ident)
            );
            fields.push(FieldNode {
                name,
                type_name: last_segment(&field.ty)
            });
        }
    }
    TypeNode {
        name: definition.ident.to_string(),
        source,
        range: byte_range(&bytes),
        kind: TypeKind::Enum,
        fields
    }
}

/// Builds an [`ImplNode`] from a parsed impl block, or [`None`] when the
/// self type is not a nameable path (`impl Trait for &T`, trait objects).
fn impl_node(source: usize, definition: &syn::ItemImpl) -> Option<ImplNode> {
    let syn::Type::Path(self_type) = definition.self_ty.as_ref() else {
        return None;
    };
    let type_name = self_type.path.segments.last()?.ident.to_string();
    let trait_name = definition
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .map(|segment| segment.ident.to_string());
    let methods = definition
        .items
        .iter()
        .filter_map(|item| {
            let syn::ImplItem::Fn(function) = item else {
                return None;
            };
            Some(MethodNode {
                name:      function.sig.ident.to_string(),
                type_refs: body_type_refs(&function.block)
            })
        })
        .collect();
    Some(ImplNode {
        type_name,
        trait_name,
        source,
        methods
    })
}

/// Builds a [`TraitNode`] from a parsed trait definition.
fn trait_node(source: usize, definition: &syn::ItemTrait) -> TraitNode {
    let methods = definition
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect();
    TraitNode {
        name: definition.ident.to_string(),
        source,
        methods
    }
}

/// Collects the first segments of qualified paths in a method body that
/// look like type names (leading uppercase), deduplicated and sorted.
fn body_type_refs(block: &syn::Block) -> Vec<String> {
    let mut collector = TypeRefCollector::default();
    collector.visit_block(block);
    collector.refs.into_iter().collect()
}

/// Syntax visitor gathering type-looking path heads from expressions.
#[derive(Default)]
struct TypeRefCollector {
    /// Distinct type-looking path heads seen so far.
    refs: BTreeSet<String>
}

impl<'ast> Visit<'ast> for TypeRefCollector {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path.segments.len() >= 2
            && let Some(first) = path.segments.first()
        {
            let head = first.ident.to_string();
            if head.chars().next().is_some_and(char::is_uppercase) {
                self.refs.insert(head);
            }
        }
        syn::visit::visit_path(self, path);
    }
}

/// Renders the last path segment of a type, or a placeholder for non-path
/// types such as references, tuples, and slices.
fn last_segment(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map_or_else(|| "<type>".to_owned(), |segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => last_segment(&reference.elem),
        _ => "<type>".to_owned()
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
