// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Human-readable terminal renderer.
//!
//! Renders each finding on three lines: a colored severity badge plus the
//! rule id and short message, the file location with the byte range, and the
//! rationale URI for follow-up. Color is emitted via `anstyle`; the caller
//! controls whether color is active via the [`TtyRenderer::with_color`]
//! flag — typical wiring routes the writer through `anstream::AutoStream`
//! and honors `NO_COLOR` outside this renderer.

use std::io::Write;

use anstyle::{AnsiColor, Color, Style};
use rustmanifest_schema::{Finding, Severity};

use crate::{Renderer, error::ReportError};

/// Human-readable terminal renderer.
pub struct TtyRenderer<W: Write> {
    writer:    W,
    use_color: bool
}

impl<W: Write> TtyRenderer<W> {
    /// Creates a renderer writing to `writer`. Color enabled by default.
    pub const fn new(writer: W) -> Self {
        Self {
            writer,
            use_color: true
        }
    }

    /// Sets whether color escape codes are emitted.
    #[must_use]
    pub const fn with_color(mut self, enabled: bool) -> Self {
        self.use_color = enabled;
        self
    }

    /// Returns ownership of the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    fn paint(&self, style: Style, text: &str) -> String {
        if self.use_color {
            format!("{style}{text}{style:#}")
        } else {
            text.to_owned()
        }
    }
}

impl<W: Write> Renderer for TtyRenderer<W> {
    fn render(&mut self, findings: &[Finding]) -> Result<(), ReportError> {
        if findings.is_empty() {
            let dim = Style::new().dimmed();
            let line = self.paint(dim, "no findings");
            writeln!(self.writer, "{line}")?;
            return Ok(());
        }

        for finding in findings {
            let badge_style = style_for(finding.severity);
            let badge = self.paint(badge_style, label_for(finding.severity));
            let rule_style = Style::new().bold();
            let rule = self.paint(rule_style, &finding.rule_id);
            let dim = Style::new().dimmed();

            writeln!(self.writer, "{badge} {rule}  {}", finding.message)?;
            let location_line = format!(
                "  {} {}:{}..{}",
                self.paint(dim, "→"),
                finding.location.file.display(),
                finding.location.range.start,
                finding.location.range.end
            );
            writeln!(self.writer, "{location_line}")?;
            let rationale_line = format!("  {} {}", self.paint(dim, "↳"), finding.rationale_uri);
            writeln!(self.writer, "{rationale_line}")?;
        }
        let summary = format!("{} finding(s) reported", findings.len());
        writeln!(self.writer, "{}", self.paint(Style::new().bold(), &summary))?;
        Ok(())
    }
}

const fn style_for(severity: Severity) -> Style {
    let color = match severity {
        Severity::Error => AnsiColor::Red,
        Severity::Warning => AnsiColor::Yellow,
        Severity::Info => AnsiColor::Cyan,
        Severity::Hint => AnsiColor::Blue
    };
    Style::new().fg_color(Some(Color::Ansi(color))).bold()
}

const fn label_for(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error  ",
        Severity::Warning => "warning",
        Severity::Info => "info   ",
        Severity::Hint => "hint   "
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

    use rustmanifest_schema::{Location, Range};

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
    fn renders_tty_without_color() {
        let mut buffer = Vec::new();
        let mut renderer = TtyRenderer::new(&mut buffer).with_color(false);
        renderer.render(&fixture()).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        insta::assert_snapshot!("tty_no_color_single_finding", output);
    }

    #[test]
    fn renders_empty_findings() {
        let mut buffer = Vec::new();
        let mut renderer = TtyRenderer::new(&mut buffer).with_color(false);
        renderer.render(&[]).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        insta::assert_snapshot!("tty_no_color_empty", output);
    }
}
