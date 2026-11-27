// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `explain` subcommand: prints a rule's identity, severity, tier, and
//! the methodology excerpt its rationale URI references.

use std::{io::Write, process::ExitCode};

use clap::Parser;
use masterror::ResultExt as _;
use rustmanifest_rules_core::{RULES, methodology};
use rustmanifest_schema::{RuleDefinition, Severity};

use crate::EXIT_CLEAN;

/// Arguments for the `explain` subcommand.
#[derive(Debug, Parser)]
pub struct ExplainArgs {
    /// Rule identifier to explain, e.g. `RM-SEC-001`.
    pub rule_id: String
}

/// Lowercase wire name of a severity, matching the serialized form.
const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
        Severity::Hint => "hint"
    }
}

/// Human-readable name of the analysis tier a rule definition belongs to.
const fn tier_name(definition: &RuleDefinition) -> &'static str {
    match definition {
        RuleDefinition::Pattern {
            ..
        } => "pattern",
        RuleDefinition::Ast {
            ..
        } => "ast",
        RuleDefinition::Semantic {
            ..
        } => "semantic"
    }
}

/// Prints the rationale for one rule: identity, severity, tier, and the
/// methodology excerpt referenced by its rationale URI.
pub(crate) fn run_explain(args: &ExplainArgs) -> masterror::AppResult<ExitCode> {
    let Some(rule) = RULES.iter().find(|rule| rule.id == args.rule_id) else {
        let known: Vec<&str> = RULES.iter().map(|rule| rule.id.as_str()).collect();
        return Err(masterror::AppError::internal(format!(
            "unknown rule id {:?}; known rules: {}",
            args.rule_id,
            known.join(", ")
        )));
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{} — {}", rule.id, rule.title).context("writing explain output")?;
    writeln!(handle, "severity: {}", severity_name(rule.severity))
        .context("writing explain output")?;
    writeln!(handle, "tier: {}", tier_name(&rule.definition)).context("writing explain output")?;
    writeln!(handle, "rationale: {}", rule.rationale_uri).context("writing explain output")?;
    if let Some(excerpt) = methodology::rationale_excerpt(&rule.rationale_uri) {
        writeln!(handle).context("writing explain output")?;
        writeln!(handle, "{excerpt}").context("writing explain output")?;
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
