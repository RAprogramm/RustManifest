// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Parameter payloads accepted by the review tools.
//!
//! Every payload is deserialized from the MCP request arguments and
//! carries a JSON schema so clients can discover the tool surface.

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for tools that accept a single workspace path.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathParams {
    /// File or directory path resolved against the workspace root.
    pub path: String
}

/// Parameters for `project_overview`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OverviewParams {
    /// File or directory path resolved against the workspace root.
    pub path: String,
    /// Cap for every ranked list in the report; defaults to 10.
    pub top:  Option<usize>
}

/// Parameters for `function_facts`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactsParams {
    /// File or directory path resolved against the workspace root.
    pub path:   String,
    /// Only include functions whose qualified name contains this
    /// substring.
    pub filter: Option<String>
}

/// Parameters for `deduce_trait`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeduceParams {
    /// File or directory path resolved against the workspace root.
    pub path:       String,
    /// Name of the workspace type the deduction is about.
    pub type_name:  String,
    /// Name of the trait to deduce, e.g. `Display` or `Default`.
    pub trait_name: String
}

/// Parameters for `whence_value`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct WhenceParams {
    /// File or directory path resolved against the workspace root.
    pub path:     String,
    /// Function asked about, by qualified or bare name.
    pub function: String
}

/// Parameters for `why_kept`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct KeptParams {
    /// File or directory path resolved against the workspace root.
    pub path:  String,
    /// The store asked about: its `Owner.field` key, its owner, its
    /// field, or its record type.
    pub store: String
}

/// Parameters for `explain_rule`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RuleIdParams {
    /// Rule identifier, e.g. `RM-STRUCT-001`.
    pub rule_id: String
}

/// Parameters for `check_rule`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RulePathParams {
    /// Rule identifier, e.g. `RM-SEC-001`.
    pub rule_id: String,
    /// File or directory path resolved against the workspace root.
    pub path:    String
}
