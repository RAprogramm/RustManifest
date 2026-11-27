// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Severity ranking, the reporting filter, and configured overrides.

use std::collections::HashMap;

use clap::ValueEnum;
use rustmanifest_config::Effective;
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, Severity};

/// Minimum severity reported in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SeverityFilter {
    /// Report `error` only.
    Error,
    /// Report `error` and `warning`.
    Warning,
    /// Report `error`, `warning`, and `info`.
    Info,
    /// Report every severity.
    Hint
}

impl SeverityFilter {
    const fn level(self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Info => 1,
            Self::Hint => 0
        }
    }

    pub(super) const fn admits(self, severity: Severity) -> bool {
        severity_rank(severity) >= self.level()
    }
}

/// Numeric rank of a severity; higher means more severe.
pub(super) const fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 3,
        Severity::Warning => 2,
        Severity::Info => 1,
        Severity::Hint => 0
    }
}

/// Rewrites each finding's severity to the effective severity of its rule.
pub(super) fn apply_severity_overrides(findings: &mut [Finding], effective: &Effective) {
    let severities: HashMap<&str, Severity> = RULES
        .iter()
        .map(|rule| (rule.id.as_str(), effective.severity_for(rule)))
        .collect();
    for finding in findings {
        if let Some(severity) = severities.get(finding.rule_id.as_str()) {
            finding.severity = *severity;
        }
    }
}
