// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Unused-pragma post-pass and its engine-synthesized rule vocabulary.
//!
//! Analyzers record the line index of every inline pragma that suppressed
//! one of their occurrences into the per-file usage recorder. This stage
//! runs once the analyzer set is exhausted for a file and turns every
//! pragma left unrecorded into a [`UNUSED_PRAGMA_RULE_ID`] finding carrying
//! a machine-applicable fix that deletes the stale pragma line.

use rustmanifest_schema::{Applicability, Finding, Fix, Location, Range, Severity, TraceStep};

use super::Orchestrator;
use crate::{pragma, source::Source};

/// Rule id of the engine-synthesized unused-pragma finding.
///
/// This rule does not live in any rules pack; the orchestrator emits it for
/// every inline pragma that suppressed nothing during a run.
pub const UNUSED_PRAGMA_RULE_ID: &str = "RM-META-001";

/// Rationale URI attached to unused-pragma findings.
pub const UNUSED_PRAGMA_RATIONALE_URI: &str = "rustmanifest://methodology/quick-reference";

impl Orchestrator {
    /// Emits one [`UNUSED_PRAGMA_RULE_ID`] finding per pragma in `source`
    /// that suppressed nothing during this run.
    ///
    /// Analyzers record the line index of every pragma that suppressed one
    /// of their occurrences into the source's usage recorder; a pragma left
    /// unrecorded either references a rule that produced no occurrence on
    /// the covered lines or names a rule unknown to this orchestrator's
    /// rule set. The message distinguishes the two cases.
    pub(super) fn unused_pragma_findings(&self, source: &Source) -> Vec<Finding> {
        let text = source.text();
        let pragmas = source.pragmas();
        if pragmas.is_empty() {
            return Vec::new();
        }
        let used = source.used_pragma_lines();
        let mut findings = Vec::new();
        for unused in pragmas
            .iter()
            .filter(|item| !used.contains(&item.line_index))
        {
            let known = self
                .analyzers
                .iter()
                .any(|analyzer| analyzer.rule().id == unused.rule_id);
            let message = if known {
                format!(
                    "unused pragma: rule {} did not fire on the suppressed lines; reason was \"{}\"",
                    unused.rule_id, unused.reason
                )
            } else {
                format!(
                    "unused pragma: references unknown rule {}; reason was \"{}\"",
                    unused.rule_id, unused.reason
                )
            };
            let (start, end) = pragma::byte_span(text, unused);
            let pragma_text = text.get(start..end).unwrap_or_default().trim();
            let step_reason = if known {
                format!(
                    "rule {} did not fire on the suppressed lines",
                    unused.rule_id
                )
            } else {
                format!("references unknown rule {}", unused.rule_id)
            };
            findings.push(Finding {
                rule_id: UNUSED_PRAGMA_RULE_ID.to_owned(),
                severity: Severity::Warning,
                location: Location {
                    file:  source.path().to_path_buf(),
                    range: Range {
                        start: u32::try_from(start).unwrap_or(u32::MAX),
                        end:   u32::try_from(end).unwrap_or(u32::MAX)
                    }
                },
                message,
                rationale_uri: UNUSED_PRAGMA_RATIONALE_URI.to_owned(),
                trace: vec![TraceStep {
                    fact:   format!("pragma `{pragma_text}` at line {}", unused.line_index + 1),
                    reason: step_reason
                }],
                fixes: vec![Fix {
                    file:          None,
                    rename_to:     None,
                    range:         line_range(text, unused.line_index),
                    replacement:   String::new(),
                    applicability: Applicability::MachineApplicable
                }]
            });
        }
        findings
    }
}

/// Byte range of one whole line of `text`, trailing newline included, so
/// replacing it with nothing deletes the line.
fn line_range(text: &str, line_index: usize) -> Range {
    let mut start = 0usize;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let end = start + line.len();
        if index == line_index {
            return Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(end).unwrap_or(u32::MAX)
            };
        }
        start = end;
    }
    Range {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end:   u32::try_from(start).unwrap_or(u32::MAX)
    }
}
