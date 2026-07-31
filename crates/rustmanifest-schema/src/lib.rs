// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical types and JSON Schemas for the `rustmanifest` ecosystem.
//!
//! This crate is the single source of truth for the on-the-wire shapes shared
//! between the engine, CLI, MCP server, LSP server, and external consumers.
//! All types derive [`schemars::JsonSchema`] so that the corresponding JSON
//! Schemas can be exported deterministically and committed under
//! `schemas/`, gated by CI against drift.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single review rule definition produced by the rules pack.
///
/// The analysis tier is implicit in the [`RuleDefinition`] variant carried by
/// the `definition` field; there is no separate `tier` enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Rule {
    /// Stable rule identifier, e.g. `RM-SEC-001`.
    pub id:            String,
    /// Default severity declared by the rules pack.
    pub severity:      Severity,
    /// Short human-readable title.
    pub title:         String,
    /// `rustmanifest://` URI pointing to the rationale section.
    pub rationale_uri: String,
    /// Tier-specific definition (pattern, AST, or semantic).
    pub definition:    RuleDefinition
}

/// Tier-specific configuration carried inside a [`Rule`].
///
/// The enum uses **external tagging**: the variant name appears as the only
/// key of the outer object on the wire, and the variant's payload is the
/// nested value. This maps naturally to TOML's `[definition.pattern]` table
/// syntax while remaining unambiguous in JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", rename_all_fields = "kebab-case")]
pub enum RuleDefinition {
    /// Tier 1 — text-level pattern scan via regular expression.
    Pattern {
        /// Regular expression matched against file contents.
        regex:         String,
        /// Glob patterns of files the rule MUST NOT inspect.
        #[serde(default)]
        exclude_globs: Vec<String>
    },
    /// Tier 2 — `syn` AST traversal identified by a built-in check name.
    Ast {
        /// Identifier of the built-in AST check implementation.
        check: String
    },
    /// Tier 3 — semantic analysis identified by a built-in check name.
    Semantic {
        /// Identifier of the built-in semantic check implementation.
        check: String
    }
}

/// Severity of a rule or a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Blocking — fails CI.
    Error,
    /// Non-blocking concern that should be addressed.
    Warning,
    /// Informational.
    Info,
    /// Style hint.
    Hint
}

/// A finding emitted by the engine for one rule against one source location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Finding {
    /// Identifier of the rule that produced this finding.
    pub rule_id:       String,
    /// Effective severity for this finding (may be overridden by user config).
    pub severity:      Severity,
    /// Source location where the rule fired.
    pub location:      Location,
    /// Human-readable message about this occurrence.
    pub message:       String,
    /// `rustmanifest://` URI of the rationale section.
    pub rationale_uri: String
}

/// Source location of a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Location {
    /// Workspace-relative file path.
    pub file:  PathBuf,
    /// Byte range within the file.
    pub range: Range
}

/// Half-open byte range within a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Range {
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end:   u32
}

/// User-supplied configuration from `rustmanifest.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct Config {
    /// Profile to apply (`strict`, `default`, `minimal`, `ci`).
    pub profile:   Option<String>,
    /// Glob patterns of files to analyze.
    pub include:   Vec<String>,
    /// Glob patterns of files to exclude.
    pub exclude:   Vec<String>,
    /// Per-rule severity overrides.
    pub overrides: Vec<RuleOverride>
}

/// Per-rule severity override entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RuleOverride {
    /// Rule identifier to override.
    pub rule_id:  String,
    /// New severity for the rule.
    pub severity: Severity
}

/// Reference to a methodology document or section embedded in the rules pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MethodologyResource {
    /// Canonical `rustmanifest://` URI.
    pub uri:         String,
    /// Resource title.
    pub title:       String,
    /// Original source path inside the repository.
    pub source_path: String,
    /// Document content in Markdown (English).
    pub content:     String
}
