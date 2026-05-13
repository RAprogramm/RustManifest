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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Rule {
    /// Stable rule identifier, e.g. `RM-SEC-001`.
    pub id:            String,
    /// Analysis tier this rule belongs to.
    pub tier:          Tier,
    /// Default severity declared by the rules pack.
    pub severity:      Severity,
    /// Short human-readable title.
    pub title:         String,
    /// `rustmanifest://` URI pointing to the rationale section.
    pub rationale_uri: String
}

/// Analysis tier indicating the cost and precision class of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Pattern scan (regex / aho-corasick). Fast, high recall, medium
    /// precision.
    Pattern,
    /// `syn`-based AST traversal. Structural and local semantic checks.
    Ast,
    /// Full semantic analysis (rust-analyzer / cargo integrations). Slow and
    /// precise.
    Semantic
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
