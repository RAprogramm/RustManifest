// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Conversion of findings into the SARIF document model.
//!
//! Findings are folded twice: once into the deduplicated rule declarations of
//! the tool driver, and once into the results carrying severity, message,
//! property bag and physical location.

use std::collections::BTreeMap;

use rustmanifest_schema::{Finding, Severity};

use super::document::{
    ArtifactLocation, Driver, MultiformatMessage, PhysicalLocation, Region, ResultProperties,
    RuleDescriptor, Run, SARIF_SCHEMA_URI, SARIF_VERSION, SarifLocation, SarifLog, SarifResult,
    TOOL_INFORMATION_URI, TOOL_NAME, Tool, TraceStepProperty
};

/// Assembles the whole SARIF log of one run from the given findings.
pub(super) fn build_log(findings: &[Finding], tool_version: &str) -> SarifLog {
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

/// Maps a finding severity onto the SARIF result level.
const fn level_of(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
        Severity::Hint => "none"
    }
}
