// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace type graph: structures, their fields, and aggregation edges.
//!
//! The graph records what each structure is made of and which other
//! workspace structures its fields refer to, giving semantic checks the
//! material for relationship-level reasoning: "`User` aggregates
//! `Credentials` through field `credentials`".

use rustmanifest_schema::Range;

use crate::source::Source;

/// One named field of a structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldNode {
    /// Field name as written.
    pub name: String,
    /// Last path segment of the field's type (`Vec<Credentials>` → `Vec`).
    pub type_name: String,
}

/// One structure definition found in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeNode {
    /// Structure name.
    pub name: String,
    /// Index of the source the structure was parsed from.
    pub source: usize,
    /// Byte range of the structure's identifier.
    pub range: Range,
    /// Named fields in declaration order; empty for tuple and unit structs.
    pub fields: Vec<FieldNode>,
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

/// All structure definitions of the analyzed workspace slice.
#[derive(Debug, Default)]
pub struct TypeGraph {
    types: Vec<TypeNode>,
}

impl TypeGraph {
    /// Builds the graph by parsing every source; files that do not parse as
    /// Rust are skipped.
    #[must_use]
    pub fn from_sources(sources: &[Source]) -> Self {
        let mut types = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let Ok(file) = syn::parse_file(source.text()) else {
                continue;
            };
            let mut items = Vec::new();
            crate::ast::flattened(&file.items, &mut items);
            for item in items {
                let syn::Item::Struct(definition) = item else {
                    continue;
                };
                types.push(node(index, definition));
            }
        }
        Self { types }
    }

    /// Structures in deterministic discovery order.
    #[must_use]
    pub fn types(&self) -> &[TypeNode] {
        &self.types
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
                    name: ident.to_string(),
                    type_name: last_segment(&field.ty),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    TypeNode {
        name: definition.ident.to_string(),
        source,
        range: Range {
            start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
            end: u32::try_from(bytes.end).unwrap_or(u32::MAX),
        },
        fields,
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
        _ => "<type>".to_owned(),
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
                    (*text).to_owned(),
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
            "struct User { identity: Identity }",
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
            "struct View<'a> { identity: &'a Identity }",
        ]);
        assert_eq!(graph.aggregators_of("Identity").len(), 1);
    }

    #[test]
    fn unparsable_files_are_skipped() {
        let graph = graph(&["struct Ok { a: u8 }", "fn broken("]);
        assert_eq!(graph.types().len(), 1);
    }

    #[test]
    fn nested_module_structs_are_recorded() {
        let graph = graph(&["mod inner { pub struct Deep { a: u8 } }"]);
        assert!(graph.type_named("Deep").is_some());
    }
}
