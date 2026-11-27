// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Inline pragma parser for per-line rule suppression.
//!
//! Pragma syntax:
//!
//! ```text
//! // rustmanifest: allow(RM-RULE-ID) reason="why this is fine"
//! ```
//!
//! The pragma applies to the line it lives on **and** to the immediately
//! following non-blank line. The `reason="..."` clause is mandatory; a
//! pragma without a reason is ignored by the parser.
//!
//! A pragma that parses but suppresses nothing during a run is flagged by
//! the orchestrator as an engine-synthesized `RM-META-001` finding: each
//! analyzer records the line index of every pragma that suppressed one of
//! its occurrences into the per-file [`Source`] usage recorder, and the
//! orchestrator turns every unrecorded pragma into a finding after all
//! analyzers ran on the file.
//!
//! [`Source`]: crate::source::Source

use std::sync::LazyLock;

use regex::Regex;

/// One parsed pragma occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pragma {
    /// Rule id named by the pragma.
    pub rule_id:    String,
    /// Justifying reason supplied via `reason="..."`.
    pub reason:     String,
    /// 0-based line index where the pragma was found.
    pub line_index: usize
}

static PRAGMA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        //\s*rustmanifest\s*:\s*allow
        \s*\(\s*
        (?P<rule>[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)
        \s*\)\s*
        reason\s*=\s*"(?P<reason>[^"]+)"
        "#
    )
    .unwrap_or_else(|err| {
        unreachable!("pragma regex must compile: {err}");
    })
});

/// Parses all pragmas from a source text.
///
/// Returns an empty vector if no pragmas are found.
#[must_use]
pub fn collect(text: &str) -> Vec<Pragma> {
    let mut pragmas = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if let Some(captures) = PRAGMA_RE.captures(line) {
            let rule_id = captures
                .name("rule")
                .map_or_default(|m| m.as_str().to_owned());
            let reason = captures
                .name("reason")
                .map_or_default(|m| m.as_str().to_owned());
            if !rule_id.is_empty() && !reason.is_empty() {
                pragmas.push(Pragma {
                    rule_id,
                    reason,
                    line_index: idx
                });
            }
        }
    }
    pragmas
}

/// Returns true if the byte offset is suppressed by a pragma for `rule_id`.
///
/// A pragma suppresses occurrences on its own line and on the next non-blank
/// line.
#[must_use]
pub fn is_suppressed(text: &str, byte_offset: usize, rule_id: &str, pragmas: &[Pragma]) -> bool {
    suppressing(text, byte_offset, rule_id, pragmas).is_some()
}

/// Returns the pragma that suppresses `rule_id` at `byte_offset`, if any.
///
/// Analyzers use the returned pragma to record its line index into the
/// per-file usage recorder, so the orchestrator can later report pragmas
/// that suppressed nothing.
pub(crate) fn suppressing<'p>(
    text: &str,
    byte_offset: usize,
    rule_id: &str,
    pragmas: &'p [Pragma]
) -> Option<&'p Pragma> {
    if pragmas.is_empty() {
        return None;
    }
    let target_line = line_of_byte(text, byte_offset);
    pragmas.iter().find(|pragma| {
        pragma.rule_id == rule_id
            && (pragma.line_index == target_line || pragma.line_index + 1 == target_line)
    })
}

/// Returns the byte range of the pragma text on the pragma's own line.
///
/// Falls back to the whole line (without its terminator) if the pragma text
/// cannot be re-matched, which cannot happen for pragmas produced by
/// [`collect`] from the same `text`.
pub(crate) fn byte_span(text: &str, pragma: &Pragma) -> (usize, usize) {
    let mut line_start = 0usize;
    for (idx, line) in text.split_inclusive('\n').enumerate() {
        if idx == pragma.line_index {
            let content = line.trim_end_matches(['\n', '\r']);
            return PRAGMA_RE
                .find(content)
                .map_or((line_start, line_start + content.len()), |found| {
                    (line_start + found.start(), line_start + found.end())
                });
        }
        line_start += line.len();
    }
    (line_start, line_start)
}

fn line_of_byte(text: &str, byte_offset: usize) -> usize {
    let mut current = 0usize;
    for (idx, line) in text.split_inclusive('\n').enumerate() {
        let next = current + line.len();
        if byte_offset < next {
            return idx;
        }
        current = next;
    }
    text.lines().count().saturating_sub(1)
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

    #[test]
    fn parses_simple_pragma() {
        let text = r#"// rustmanifest: allow(RM-SEC-001) reason="test fixture intentionally hardcodes a credential"
let password = "hunter2";
"#;
        let pragmas = collect(text);
        assert_eq!(pragmas.len(), 1);
        assert_eq!(pragmas[0].rule_id, "RM-SEC-001");
        assert!(pragmas[0].reason.contains("test fixture"));
    }

    #[test]
    fn ignores_pragma_without_reason() {
        let text = "// rustmanifest: allow(RM-SEC-001)\nlet password = \"x\";\n";
        assert_eq!(collect(text), [] as [crate::pragma::Pragma; 0]);
    }

    #[test]
    fn suppresses_next_line_finding() {
        let text = "// rustmanifest: allow(RM-SEC-001) reason=\"ok\"\nlet password = \"x\";\n";
        let pragmas = collect(text);
        let offset = text.find("let password").unwrap();
        assert!(is_suppressed(text, offset, "RM-SEC-001", &pragmas));
    }

    #[test]
    fn suppressing_returns_the_matching_pragma() {
        let text = "// rustmanifest: allow(RM-SEC-001) reason=\"ok\"\nlet password = \"x\";\n";
        let pragmas = collect(text);
        let offset = text.find("let password").unwrap();
        let found = suppressing(text, offset, "RM-SEC-001", &pragmas).unwrap();
        assert_eq!(found.line_index, 0);
        assert_eq!(found.rule_id, "RM-SEC-001");
    }

    #[test]
    fn byte_span_covers_exact_pragma_text() {
        let text = "fn main() {}\n// rustmanifest: allow(RM-SEC-001) reason=\"ok\"\nlet x = 1;\n";
        let pragmas = collect(text);
        let (start, end) = byte_span(text, &pragmas[0]);
        assert_eq!(
            &text[start..end],
            "// rustmanifest: allow(RM-SEC-001) reason=\"ok\""
        );
    }

    #[test]
    fn does_not_suppress_different_rule() {
        let text = "// rustmanifest: allow(RM-RUST-001) reason=\"ok\"\nlet password = \"x\";\n";
        let pragmas = collect(text);
        let offset = text.find("let password").unwrap();
        assert!(!is_suppressed(text, offset, "RM-SEC-001", &pragmas));
    }
}
