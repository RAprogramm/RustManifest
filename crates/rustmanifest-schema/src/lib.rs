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

/// One step of the machine-readable derivation trace of a [`Finding`].
///
/// A step pairs an observed fact (what the engine saw in the analyzed
/// sources) with the reason that fact contributes to the finding. The
/// ordered step list reconstructs how the finding was derived without
/// parsing the human-readable message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TraceStep {
    /// Observed evidence, e.g. an offending item with its file.
    pub fact:   String,
    /// Why the fact contributes to the finding.
    pub reason: String
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
    pub rationale_uri: String,
    /// Machine-readable derivation trace; absent on the wire when empty, so
    /// payloads produced before this field existed remain valid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace:         Vec<TraceStep>
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    fn sample_finding(trace: Vec<TraceStep>) -> Finding {
        Finding {
            rule_id: "RM-SEC-001".to_owned(),
            severity: Severity::Error,
            location: Location {
                file:  PathBuf::from("src/lib.rs"),
                range: Range {
                    start: 10,
                    end:   25
                }
            },
            message: "sample".to_owned(),
            rationale_uri: "rustmanifest://methodology/security#creds".to_owned(),
            trace
        }
    }

    #[test]
    fn finding_without_trace_round_trips_and_omits_the_field() {
        let finding = sample_finding(Vec::new());
        let json = serde_json::to_string(&finding).unwrap();
        assert!(!json.contains("\"trace\""));
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, finding);
    }

    #[test]
    fn finding_with_trace_round_trips() {
        let finding = sample_finding(vec![TraceStep {
            fact:   "bytes 10..25 in src/lib.rs".to_owned(),
            reason: "regex of RM-SEC-001 matched".to_owned()
        }]);
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("\"trace\""));
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, finding);
    }

    #[test]
    fn pre_trace_payload_still_deserializes() {
        let json = r#"{
            "rule-id": "RM-SEC-001",
            "severity": "error",
            "location": { "file": "src/lib.rs", "range": { "start": 10, "end": 25 } },
            "message": "sample",
            "rationale-uri": "rustmanifest://methodology/security#creds"
        }"#;
        let finding: Finding = serde_json::from_str(json).unwrap();
        assert!(finding.trace.is_empty());
    }
}
