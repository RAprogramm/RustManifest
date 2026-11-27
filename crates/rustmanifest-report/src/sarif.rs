// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! SARIF 2.1.0 renderer.
//!
//! The document model comes from the crate that generates it out of the
//! format's published schema, so a field the format defines is a field this
//! renderer can state. The hand-rolled model that stood here carried only
//! what somebody had needed so far, and what it left out was the line
//! number code-scanning ingestion keys on.

mod conversion;

use std::io::Write;

use rustmanifest_schema::Finding;

use crate::{Renderer, error::ReportError, sarif::conversion::build_log};

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
    fn render(&mut self, findings: &[Finding]) -> Result<(), masterror::AppError> {
        let log = build_log(findings, &self.tool_version);
        serde_json::to_writer_pretty(&mut self.writer, &log).map_err(ReportError::from)?;
        self.writer.write_all(b"\n").map_err(ReportError::from)?;
        Ok(())
    }
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

    use rustmanifest_schema::{Location, Range, Severity, TraceStep};

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
