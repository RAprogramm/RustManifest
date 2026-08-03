// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! SARIF 2.1.0 document model.
//!
//! Only the subset consumed by GitHub code scanning and comparable ingesters
//! is modelled: the log envelope, the tool driver with its declared rules, and
//! results carrying byte-precise physical locations.

use serde::Serialize;

/// SARIF specification version stamped into every emitted log.
pub(super) const SARIF_VERSION: &str = "2.1.0";
/// Canonical location of the SARIF 2.1.0 JSON schema.
pub(super) const SARIF_SCHEMA_URI: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
/// Driver name reported to the ingester.
pub(super) const TOOL_NAME: &str = "rustmanifest";
/// Driver home page reported to the ingester.
pub(super) const TOOL_INFORMATION_URI: &str = "https://github.com/RAprogramm/RustManifest";

/// Top-level SARIF log envelope.
#[derive(Serialize)]
pub(super) struct SarifLog {
    pub(super) version: String,
    #[serde(rename = "$schema")]
    pub(super) schema:  String,
    pub(super) runs:    Vec<Run>
}

/// One analysis run inside a log.
#[derive(Serialize)]
pub(super) struct Run {
    pub(super) tool:    Tool,
    pub(super) results: Vec<SarifResult>
}

/// Tool section of a run.
#[derive(Serialize)]
pub(super) struct Tool {
    pub(super) driver: Driver
}

/// Driver metadata and the rules it declares.
#[derive(Serialize)]
pub(super) struct Driver {
    pub(super) name:            String,
    pub(super) version:         String,
    #[serde(rename = "informationUri")]
    pub(super) information_uri: String,
    pub(super) rules:           Vec<RuleDescriptor>
}

/// Declaration of one rule the driver can report.
#[derive(Serialize)]
pub(super) struct RuleDescriptor {
    pub(super) id:                String,
    pub(super) name:              String,
    #[serde(rename = "shortDescription")]
    pub(super) short_description: MultiformatMessage,
    #[serde(rename = "helpUri")]
    pub(super) help_uri:          String
}

/// One reported result.
#[derive(Serialize)]
pub(super) struct SarifResult {
    #[serde(rename = "ruleId")]
    pub(super) rule_id:    String,
    pub(super) level:      &'static str,
    pub(super) message:    MultiformatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) properties: Option<ResultProperties>,
    pub(super) locations:  Vec<SarifLocation>
}

/// Property bag attached to a result.
#[derive(Serialize)]
pub(super) struct ResultProperties {
    pub(super) trace: Vec<TraceStepProperty>
}

/// One derivation step exposed through the property bag.
#[derive(Serialize)]
pub(super) struct TraceStepProperty {
    pub(super) fact:   String,
    pub(super) reason: String
}

/// Location wrapper of a result.
#[derive(Serialize)]
pub(super) struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    pub(super) physical_location: PhysicalLocation
}

/// Artifact and region a result points at.
#[derive(Serialize)]
pub(super) struct PhysicalLocation {
    #[serde(rename = "artifactLocation")]
    pub(super) artifact_location: ArtifactLocation,
    pub(super) region:            Region
}

/// Artifact a result points at.
#[derive(Serialize)]
pub(super) struct ArtifactLocation {
    pub(super) uri: String
}

/// Byte-precise span inside an artifact.
#[derive(Serialize)]
pub(super) struct Region {
    #[serde(rename = "byteOffset")]
    pub(super) byte_offset: u32,
    #[serde(rename = "byteLength")]
    pub(super) byte_length: u32
}

/// Message carried by rules and results.
#[derive(Serialize)]
pub(super) struct MultiformatMessage {
    pub(super) text: String
}
