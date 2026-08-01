// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! SARIF 2.1.0 renderer.
//!
//! The output schema is intentionally hand-rolled rather than imported from a
//! third-party crate. SARIF is large; this renderer emits only the fields
//! used by GitHub code scanning and other common ingesters: tool driver
//! metadata, declared rules, and results with byte-precise physical
//! locations.

use std::{collections::BTreeMap, io::Write};

use rustmanifest_schema::{Finding, Severity};
use serde::Serialize;

use crate::{Renderer, error::ReportError};

const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA_URI: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
const TOOL_NAME: &str = "rustmanifest";
const TOOL_INFORMATION_URI: &str = "https://github.com/RAprogramm/RustManifest";

/// Renders findings as a SARIF 2.1.0 document.
pub struct SarifRenderer<W: Write> {
    writer:       W,
    tool_version: String
}

impl<W: Write> SarifRenderer<W> {
    /// Creates a renderer using the given tool version string.
    pub fn new(writer: W, tool_version: impl Into<String>) -> Self {
        Self {
            writer,
            tool_version: tool_version.into()
        }
    }

    /// Returns ownership of the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> Renderer for SarifRenderer<W> {
    fn render(&mut self, findings: &[Finding]) -> Result<(), ReportError> {
        let log = build_log(findings, &self.tool_version);
        serde_json::to_writer_pretty(&mut self.writer, &log)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}

fn build_log(findings: &[Finding], tool_version: &str) -> SarifLog {
    let mut rules_index: BTreeMap<&str, RuleDescriptor> = BTreeMap::new();
    for finding in findings {
        rules_index
            .entry(finding.rule_id.as_str())
            .or_insert_with(|| RuleDescriptor {
                id:                finding.rule_id.clone(),
                name:              finding.rule_id.clone(),
                short_description: MultiformatMessage {
                    text: finding.message.clone()
                },
                help_uri:          finding.rationale_uri.clone()
            });
    }
    let rules: Vec<RuleDescriptor> = rules_index.into_values().collect();

    let results: Vec<SarifResult> = findings
        .iter()
        .map(|finding| SarifResult {
            rule_id:    finding.rule_id.clone(),
            level:      level_of(finding.severity),
            message:    MultiformatMessage {
                text: finding.message.clone()
            },
            properties: properties_of(finding),
            locations:  vec![SarifLocation {
                physical_location: PhysicalLocation {
                    artifact_location: ArtifactLocation {
                        uri: finding.location.file.display().to_string()
                    },
                    region:            Region {
                        byte_offset: finding.location.range.start,
                        byte_length: finding
                            .location
                            .range
                            .end
                            .saturating_sub(finding.location.range.start)
                    }
                }
            }]
        })
        .collect();

    SarifLog {
        version: SARIF_VERSION.to_owned(),
        schema:  SARIF_SCHEMA_URI.to_owned(),
        runs:    vec![Run {
            tool: Tool {
                driver: Driver {
                    name: TOOL_NAME.to_owned(),
                    version: tool_version.to_owned(),
                    information_uri: TOOL_INFORMATION_URI.to_owned(),
                    rules
                }
            },
            results
        }]
    }
}

/// Builds the SARIF property bag of one result: findings carrying a
/// derivation trace expose it as `properties.trace`, findings without one
/// omit the bag entirely.
fn properties_of(finding: &Finding) -> Option<ResultProperties> {
    if finding.trace.is_empty() {
        return None;
    }
    Some(ResultProperties {
        trace: finding
            .trace
            .iter()
            .map(|step| TraceStepProperty {
                fact:   step.fact.clone(),
                reason: step.reason.clone()
            })
            .collect()
    })
}

const fn level_of(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
        Severity::Hint => "none"
    }
}

#[derive(Serialize)]
struct SarifLog {
    version: String,
    #[serde(rename = "$schema")]
    schema:  String,
    runs:    Vec<Run>
}

#[derive(Serialize)]
struct Run {
    tool:    Tool,
    results: Vec<SarifResult>
}

#[derive(Serialize)]
struct Tool {
    driver: Driver
}

#[derive(Serialize)]
struct Driver {
    name:            String,
    version:         String,
    #[serde(rename = "informationUri")]
    information_uri: String,
    rules:           Vec<RuleDescriptor>
}

#[derive(Serialize)]
struct RuleDescriptor {
    id:                String,
    name:              String,
    #[serde(rename = "shortDescription")]
    short_description: MultiformatMessage,
    #[serde(rename = "helpUri")]
    help_uri:          String
}

#[derive(Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id:    String,
    level:      &'static str,
    message:    MultiformatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<ResultProperties>,
    locations:  Vec<SarifLocation>
}

#[derive(Serialize)]
struct ResultProperties {
    trace: Vec<TraceStepProperty>
}

#[derive(Serialize)]
struct TraceStepProperty {
    fact:   String,
    reason: String
}

#[derive(Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: PhysicalLocation
}

#[derive(Serialize)]
struct PhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: ArtifactLocation,
    region:            Region
}

#[derive(Serialize)]
struct ArtifactLocation {
    uri: String
}

#[derive(Serialize)]
struct Region {
    #[serde(rename = "byteOffset")]
    byte_offset: u32,
    #[serde(rename = "byteLength")]
    byte_length: u32
}

#[derive(Serialize)]
struct MultiformatMessage {
    text: String
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

    use rustmanifest_schema::{Location, Range, TraceStep};

    use super::*;

    fn fixture() -> Vec<Finding> {
        vec![Finding {
            rule_id:       "RM-SEC-001".to_owned(),
            severity:      Severity::Error,
            location:      Location {
                file:  PathBuf::from("src/lib.rs"),
                range: Range {
                    start: 10,
                    end:   25
                }
            },
            message:       "Hardcoded credentials: matched at bytes 10..25".to_owned(),
            rationale_uri: "rustmanifest://methodology/security#creds".to_owned(),
            trace:         vec![TraceStep {
                fact:   "bytes 10..25 in src/lib.rs".to_owned(),
                reason: "regex of RM-SEC-001 matched".to_owned()
            }],
            fixes:         Vec::new()
        }]
    }

    #[test]
    fn renders_sarif_log() {
        let mut buffer = Vec::new();
        let mut renderer = SarifRenderer::new(&mut buffer, "0.0.0");
        renderer.render(&fixture()).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        insta::assert_snapshot!("sarif_single_finding", output);
    }

    #[test]
    fn trace_lands_in_the_result_property_bag() {
        let mut buffer = Vec::new();
        let mut renderer = SarifRenderer::new(&mut buffer, "0.0.0");
        renderer.render(&fixture()).unwrap();
        let log: serde_json::Value = serde_json::from_slice(&buffer).unwrap();
        let trace = &log["runs"][0]["results"][0]["properties"]["trace"];
        assert_eq!(trace[0]["fact"], "bytes 10..25 in src/lib.rs");
        assert_eq!(trace[0]["reason"], "regex of RM-SEC-001 matched");
    }

    #[test]
    fn traceless_finding_omits_the_property_bag() {
        let mut findings = fixture();
        findings[0].trace = Vec::new();
        let mut buffer = Vec::new();
        let mut renderer = SarifRenderer::new(&mut buffer, "0.0.0");
        renderer.render(&findings).unwrap();
        let log: serde_json::Value = serde_json::from_slice(&buffer).unwrap();
        assert!(log["runs"][0]["results"][0].get("properties").is_none());
    }
}
