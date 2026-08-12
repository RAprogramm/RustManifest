// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Conversion of findings into the interchange format's own model.
//!
//! The shapes come from the crate that generates them out of the format's
//! published schema. A hand-rolled model stood here before, holding only
//! the fields somebody had needed so far — and it omitted the line number
//! that code-scanning ingestion keys on, so the output was well-formed and
//! unusable by its stated consumer.
//!
//! Line numbers are counted from the file where it can be read, and left
//! out where it cannot. A byte offset is exact and always available; a line
//! number is what the consumer indexes by, so both are stated.

use std::collections::BTreeMap;

use rustmanifest_schema::{Finding, Severity};
use serde_sarif::sarif::{
    ArtifactLocation, Location, Message, MultiformatMessageString, PhysicalLocation, PropertyBag,
    Region, ReportingDescriptor, Result as SarifResult, ResultLevel, Run, Sarif, Tool,
    ToolComponent, Version
};

/// Name the tool reports itself under.
const TOOL_NAME: &str = "rustmanifest";

/// Home page of the tool, as the format's driver metadata.
const TOOL_INFORMATION_URI: &str = "https://github.com/RAprogramm/RustManifest";

/// Assembles the whole log of one run from the given findings.
pub(super) fn build_log(findings: &[Finding], tool_version: &str) -> Sarif {
    let mut declared: BTreeMap<&str, ReportingDescriptor> = BTreeMap::new();
    for finding in findings {
        declared
            .entry(finding.rule_id.as_str())
            .or_insert_with(|| descriptor(finding));
    }
    let driver = ToolComponent::builder()
        .name(TOOL_NAME.to_owned())
        .version(tool_version.to_owned())
        .information_uri(TOOL_INFORMATION_URI.to_owned())
        .rules(declared.into_values().collect::<Vec<_>>())
        .build();
    let run = Run::builder()
        .tool(Tool::builder().driver(driver).build())
        .results(findings.iter().map(result_of).collect::<Vec<_>>())
        .build();
    Sarif::builder()
        .version(Version::V2_1_0.to_string())
        .schema(serde_sarif::sarif::SCHEMA_URL.to_owned())
        .runs(vec![run])
        .build()
}

/// The rule declaration one finding stands for.
fn descriptor(finding: &Finding) -> ReportingDescriptor {
    ReportingDescriptor::builder()
        .id(finding.rule_id.clone())
        .name(finding.rule_id.clone())
        .short_description(
            MultiformatMessageString::builder()
                .text(finding.message.clone())
                .build()
        )
        .help_uri(finding.rationale_uri.clone())
        .build()
}

/// One finding as one result.
fn result_of(finding: &Finding) -> SarifResult {
    let built = SarifResult::builder()
        .rule_id(finding.rule_id.clone())
        .level(level_of(finding.severity))
        .message(Message::builder().text(finding.message.clone()).build())
        .locations(vec![location_of(finding)]);
    match properties_of(finding) {
        Some(properties) => built.properties(properties).build(),
        None => built.build()
    }
}

/// Where one finding stands, by byte offset and — where the file can be
/// read — by line number.
fn location_of(finding: &Finding) -> Location {
    let start = i64::from(finding.location.range.start);
    let length = i64::from(
        finding
            .location
            .range
            .end
            .saturating_sub(finding.location.range.start)
    );
    let region = Region::builder().byte_offset(start).byte_length(length);
    let region = match lines_of(finding) {
        Some((first, last)) => region.start_line(first).end_line(last).build(),
        None => region.build()
    };
    Location::builder()
        .physical_location(
            PhysicalLocation::builder()
                .artifact_location(
                    ArtifactLocation::builder()
                        .uri(finding.location.file.display().to_string())
                        .build()
                )
                .region(region)
                .build()
        )
        .build()
}

/// The first and last line a finding's byte range covers, counted from the
/// file itself; nothing where the file cannot be read.
fn lines_of(finding: &Finding) -> Option<(i64, i64)> {
    let text = std::fs::read_to_string(&finding.location.file).ok()?;
    let counted = |offset: u32| -> i64 {
        let upto = text.get(..offset as usize).unwrap_or(&text);
        i64::try_from(upto.lines().count().max(1)).unwrap_or(1)
    };
    Some((
        counted(finding.location.range.start),
        counted(finding.location.range.end)
    ))
}

/// The derivation trace as the result's property bag, absent where the
/// finding carries none.
fn properties_of(finding: &Finding) -> Option<PropertyBag> {
    if finding.trace.is_empty() {
        return None;
    }
    let steps: Vec<serde_json::Value> = finding
        .trace
        .iter()
        .map(|step| serde_json::json!({ "fact": step.fact, "reason": step.reason }))
        .collect();
    let mut bag = PropertyBag::builder().build();
    bag.additional_properties
        .insert("trace".to_owned(), serde_json::Value::Array(steps));
    Some(bag)
}

/// The format's level for one severity.
const fn level_of(severity: Severity) -> ResultLevel {
    match severity {
        Severity::Error => ResultLevel::Error,
        Severity::Warning => ResultLevel::Warning,
        Severity::Info | Severity::Hint => ResultLevel::Note
    }
}
