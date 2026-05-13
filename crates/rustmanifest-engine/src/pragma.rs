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
//! pragma without a reason is ignored (and a future `RM-META-001` rule will
//! flag it explicitly).

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
        (?P<rule>[A-Z]+-[A-Z]+-\d{3})
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
                .map(|m| m.as_str().to_owned())
                .unwrap_or_default();
            let reason = captures
                .name("reason")
                .map(|m| m.as_str().to_owned())
                .unwrap_or_default();
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
    if pragmas.is_empty() {
        return false;
    }
    let target_line = line_of_byte(text, byte_offset);
    for pragma in pragmas {
        if pragma.rule_id != rule_id {
            continue;
        }
        if pragma.line_index == target_line {
            return true;
        }
        if pragma.line_index + 1 == target_line && !line_is_blank(text, pragma.line_index + 1) {
            // Same line, already handled above.
        }
        if pragma.line_index + 1 == target_line {
            return true;
        }
    }
    false
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

fn line_is_blank(text: &str, line_index: usize) -> bool {
    text.lines()
        .nth(line_index)
        .is_none_or(|line| line.trim().is_empty())
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
        assert!(collect(text).is_empty());
    }

    #[test]
    fn suppresses_next_line_finding() {
        let text = "// rustmanifest: allow(RM-SEC-001) reason=\"ok\"\nlet password = \"x\";\n";
        let pragmas = collect(text);
        let offset = text.find("let password").unwrap();
        assert!(is_suppressed(text, offset, "RM-SEC-001", &pragmas));
    }

    #[test]
    fn does_not_suppress_different_rule() {
        let text = "// rustmanifest: allow(RM-RUST-001) reason=\"ok\"\nlet password = \"x\";\n";
        let pragmas = collect(text);
        let offset = text.find("let password").unwrap();
        assert!(!is_suppressed(text, offset, "RM-SEC-001", &pragmas));
    }
}
