// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON renderer.

use std::io::Write;

use rustmanifest_schema::Finding;

use crate::{Renderer, error::ReportError};

/// Writes findings as a pretty-printed JSON array.
pub struct JsonRenderer<W: Write> {
    writer: W
}

impl<W: Write> JsonRenderer<W> {
    /// Creates a new renderer writing to `writer`.
    pub const fn new(writer: W) -> Self {
        Self {
            writer
        }
    }

    /// Returns ownership of the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> Renderer for JsonRenderer<W> {
    fn render(&mut self, findings: &[Finding]) -> Result<(), ReportError> {
        serde_json::to_writer_pretty(&mut self.writer, findings)?;
        self.writer.write_all(b"\n")?;
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

    use rustmanifest_schema::{Location, Range, Severity};

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
            rationale_uri: "rustmanifest://methodology/security#creds".to_owned()
        }]
    }

    #[test]
    fn renders_pretty_json() {
        let mut buffer = Vec::new();
        let mut renderer = JsonRenderer::new(&mut buffer);
        renderer.render(&fixture()).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        insta::assert_snapshot!("json_single_finding", output);
    }
}
