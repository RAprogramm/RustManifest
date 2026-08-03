// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Catalog of built-in structural checks: the identifiers a rule may name in
//! `definition.ast.check` and the check each identifier runs.

use super::{
    Occurrence, api_size, constant_encapsulation, constructor_design, delegating_constructors,
    entity_naming, fakes_over_mocks, immutability_first, method_naming
};

/// Built-in structural checks addressable from `definition.ast.check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Check {
    /// `entity-naming` — agent-suffixed type names (`STRUCTURE.md` §1).
    EntityNaming,
    /// `method-naming` — redundant `get_` accessor prefix (§2).
    MethodNaming,
    /// `api-size` — inherent impls exposing too many public methods (§4).
    ApiSize,
    /// `constructor-design` — constructors doing more than assignment (§5).
    ConstructorDesign,
    /// `delegating-constructors` — constructors duplicating `Self { .. }`
    /// instead of delegating to one primary constructor (§6).
    DelegatingConstructors,
    /// `immutability-first` — `set_*` mutators taking `&mut self` (§7).
    ImmutabilityFirst,
    /// `constant-encapsulation` — module-level `pub const` items (§8).
    ConstantEncapsulation,
    /// `fakes-over-mocks` — mock-library imports and `#[automock]` (§9).
    FakesOverMocks
}

impl Check {
    /// Resolves a check identifier from the rules pack.
    pub(super) fn resolved(name: &str) -> Option<Self> {
        match name {
            "entity-naming" => Some(Self::EntityNaming),
            "method-naming" => Some(Self::MethodNaming),
            "api-size" => Some(Self::ApiSize),
            "constructor-design" => Some(Self::ConstructorDesign),
            "delegating-constructors" => Some(Self::DelegatingConstructors),
            "immutability-first" => Some(Self::ImmutabilityFirst),
            "constant-encapsulation" => Some(Self::ConstantEncapsulation),
            "fakes-over-mocks" => Some(Self::FakesOverMocks),
            _ => None
        }
    }

    /// Runs the check over a parsed file.
    pub(super) fn occurrences(self, file: &syn::File) -> Vec<Occurrence> {
        match self {
            Self::EntityNaming => entity_naming::occurrences(file),
            Self::MethodNaming => method_naming::occurrences(file),
            Self::ApiSize => api_size::occurrences(file),
            Self::ConstructorDesign => constructor_design::occurrences(file),
            Self::DelegatingConstructors => delegating_constructors::occurrences(file),
            Self::ImmutabilityFirst => immutability_first::occurrences(file),
            Self::ConstantEncapsulation => constant_encapsulation::occurrences(file),
            Self::FakesOverMocks => fakes_over_mocks::occurrences(file)
        }
    }
}
