// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Node types of the workspace type graph: type, field, method, impl,
//! and trait definitions, plus the absorption tally for macro-generated
//! definitions. Pure data — construction lives in the builder module,
//! queries on the graph root.

use rustmanifest_schema::Range;

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
    pub name:      String,
    /// Index of the source the type was parsed from; [`usize::MAX`] for
    /// macro-generated types absorbed from expansion, which have no
    /// surface file.
    pub source:    usize,
    /// Byte range of the type's identifier; zeroed for generated types.
    pub range:     Range,
    /// Whether the node is a structure or an enum.
    pub kind:      TypeKind,
    /// Whether the type exists only in macro expansion, absent from
    /// surface syntax.
    pub generated: bool,
    /// Trait names listed in `#[derive(...)]` attributes on the
    /// definition.
    pub derives:   Vec<String>,
    /// Named fields (or enum variant payloads) in declaration order; empty
    /// for tuple and unit structs.
    pub fields:    Vec<FieldNode>
}

/// What one call to [`TypeGraph::absorb_expansion`] integrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Absorption {
    /// Derive-generated impls added to the graph.
    pub impls: u64,
    /// Macro-generated type definitions added to the graph.
    pub types: u64
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
    /// Index of the source the impl was parsed from; for derived impls
    /// absorbed from macro expansion, the source defining the type.
    pub source:     usize,
    /// Whether the impl came from a derive expansion rather than surface
    /// syntax.
    pub derived:    bool,
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
