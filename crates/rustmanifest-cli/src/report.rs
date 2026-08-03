// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `report` subcommand: everything the engine knows about a program, in
//! one pass.
//!
//! The knowledge is spread over five surfaces — what the sources hold,
//! what the program does for an observer outside the process, the facts
//! behind that, the judgements against the canonical forms, and how much
//! of the sources the engine could read at all. Running them one by one
//! makes the reader assemble the picture; this subcommand assembles it
//! and prints the honesty section last, so a reader always sees what the
//! rest of the report is worth.

use std::{fmt::Write as _, io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_config::Effective;
use rustmanifest_engine::{
    Source,
    overview::{Convention, overview_markdown}
};
use rustmanifest_learn::{LearnOptions, learn_roles};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, RuleDefinition};

use crate::EXIT_CLEAN;

/// Arguments for the `report` subcommand.
#[derive(Debug, Parser)]
pub struct ReportArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Cap for every ranked list in the report.
    #[arg(long, default_value_t = 10)]
    pub top: usize
}

/// Runs the `report` subcommand.
pub(crate) fn run_report(args: &ReportArgs) -> anyhow::Result<ExitCode> {
    let files =
        rustmanifest_engine::walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let root = args.paths.first().cloned().unwrap_or_default();
    let mut out = String::new();
    out.push_str(&behaviour(&sources, &root));
    out.push_str(&judgement(&sources, &root));
    out.push_str(&structure(&sources, args.top));
    out.push_str(&honesty(&sources));
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write!(handle, "{out}").context("writing report output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// What the sources hold: modules, types, roles, boundaries, layers, the
/// external surface, and the learned conventions.
fn structure(sources: &[Source], top: usize) -> String {
    let conventions: Vec<Convention> = learn_roles(sources, LearnOptions::default())
        .into_iter()
        .map(|clause| Convention {
            role:     clause.role,
            includes: clause.includes,
            excludes: clause.excludes
        })
        .collect();
    let rendered = overview_markdown(sources, top, &conventions);
    let mut out = String::from("\n## structure\n");
    for block in rendered.split("\n## ").skip(1) {
        let Some((heading, body)) = block.split_once('\n') else {
            continue;
        };
        let lines: Vec<&str> = body
            .lines()
            .filter(|line| line.starts_with("- "))
            .filter(|line| !empty_note(line))
            .collect();
        if lines.is_empty() {
            continue;
        }
        let _ = write!(out, "\n### {}\n\n{}\n", heading.trim(), lines.join("\n"));
    }
    out
}

/// Whether a rendered line states an absence rather than a fact. A
/// report that prints every absence buries the few lines that carry
/// content, so absences are dropped here and the coverage section says
/// what was and was not read.
fn empty_note(line: &str) -> bool {
    let text = line.trim_start_matches("- ");
    text.starts_with("no ") || text.starts_with("nothing ")
}

/// What the program does for an observer outside the process, per entry
/// point, in the words the recorded facts license.
fn behaviour(sources: &[Source], root: &std::path::Path) -> String {
    let mut out = String::from("## behaviour\n\n");
    let entries = rustmanifest_engine::program_meaning(sources);
    if entries.is_empty() {
        out.push_str("- no process entry point\n");
        return out;
    }
    for entry in &entries {
        let _ = writeln!(out, "- entry point `{}`", relative(&entry.file, root));
        for claim in &entry.claims {
            let _ = writeln!(
                out,
                "  - {} writes {} to {}{}",
                certainty(claim.execution),
                claim.payload_text(),
                claim.write.stream,
                if claim.write.multiplicity == rustmanifest_engine::Multiplicity::Unbounded {
                    ", once per iteration"
                } else {
                    ""
                }
            );
        }
    }
    out
}

/// How certainly something happens, in one word.
const fn certainty(grade: rustmanifest_engine::Grade) -> &'static str {
    match grade {
        rustmanifest_engine::Grade::Certain => "always",
        rustmanifest_engine::Grade::Possible => "maybe",
        rustmanifest_engine::Grade::Dead => "never",
        rustmanifest_engine::Grade::Unreached => "unreached"
    }
}

/// Shortens a path against the analyzed root so a line stays readable.
fn relative(path: &str, root: &std::path::Path) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let root = root.trim_end_matches('/');
    if root.is_empty() || root == "." {
        return path.trim_start_matches("./").to_owned();
    }
    path.strip_prefix(root).map_or_else(
        || path.to_owned(),
        |rest| rest.trim_start_matches('/').to_owned()
    )
}

/// Judgements against the canonical forms, each with its own evidence.
fn judgement(sources: &[Source], root: &std::path::Path) -> String {
    let mut out = String::from("\n## deviations\n\n");
    let findings = analyze(sources);
    if findings.is_empty() {
        out.push_str("- no rule of the enabled pack applies\n");
        return out;
    }
    for finding in &findings {
        let _ = writeln!(
            out,
            "- {} `{}` — {}",
            format!("{:?}", finding.severity).to_lowercase(),
            finding.rule_id,
            finding.message
        );
        for step in &finding.trace {
            let _ = writeln!(out, "  - {} [{}]", step.fact, relative(&step.reason, root));
        }
    }
    out
}

/// Runs every enabled rule of the bundled pack over the sources.
fn analyze(sources: &[Source]) -> Vec<Finding> {
    let Ok(config) = rustmanifest_config::load(None) else {
        return Vec::new();
    };
    let Ok(effective) = Effective::from_config(config) else {
        return Vec::new();
    };
    RULES
        .iter()
        .filter(|rule| effective.rule_enabled(rule))
        .filter(|rule| matches!(rule.definition, RuleDefinition::Semantic { .. }))
        .filter_map(|rule| rustmanifest_engine::SemanticAnalyzer::new(rule.clone()).ok())
        .flat_map(|analyzer| analyzer.analyze(sources))
        .collect()
}

/// How much of the sources the engine could read, printed last so the
/// reader learns what the rest of the report is worth.
fn honesty(sources: &[Source]) -> String {
    let coverage = rustmanifest_engine::coverage(sources);
    let mut out = format!(
        "\n## coverage\n\n- files parsed: {}/{} ({:.1}%)\n",
        coverage.files_parsed,
        coverage.files_total,
        coverage.parse_rate()
    );
    if coverage.call_candidates_total == 0 {
        out.push_str("- calls seen: 0\n");
    } else {
        let _ = writeln!(
            out,
            "- calls resolved in-workspace: {}/{}, receivers left unknown: {}",
            coverage.call_candidates_resolved,
            coverage.call_candidates_total,
            coverage.call_candidates_unknown
        );
    }
    out.push_str("- unread sources are not a claim of absence\n");
    out
}
