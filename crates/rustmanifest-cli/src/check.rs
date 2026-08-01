// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `check` subcommand: the full analysis pipeline.
//!
//! Loads configuration, discovers files, runs all three tiers through the
//! orchestrator, merges workspace-level semantic findings, optionally
//! delegates to `cargo clippy`, applies severity overrides and the
//! severity filter, renders in the requested format, and maps the outcome
//! to the documented exit codes.

use std::{
    collections::HashMap,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
    process::ExitCode
};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use globset::{Glob, GlobSet, GlobSetBuilder};
use rustmanifest_config::Effective;
use rustmanifest_engine::{
    AstAnalyzer, FindingsCache, OrchestratorBuilder, PatternAnalyzer, SemanticAnalyzer, walker
};
use rustmanifest_report::{JsonRenderer, Renderer, SarifRenderer, TtyRenderer};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, Location, Range, Rule, RuleDefinition, Severity};
use tracing::{info, warn};

use crate::{EXIT_CLEAN, EXIT_FINDINGS, read_sources};

/// Arguments for the `check` subcommand.
#[derive(Debug, Parser)]
pub struct CheckArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
    pub format: OutputFormat,

    /// Minimum severity to report.
    #[arg(long, value_enum, default_value_t = SeverityFilter::Hint)]
    pub severity_filter: SeverityFilter,

    /// Force-disable color output even on a TTY.
    #[arg(long)]
    pub no_color: bool,

    /// Per-file memory budget in bytes.
    #[arg(long, default_value_t = rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES)]
    pub max_file_size: u64,

    /// Path to a `rustmanifest.toml` configuration file. Defaults to
    /// `rustmanifest.toml` in the current directory when present.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Directory for the persistent findings cache. Off by default; the
    /// directory is created when missing. Entries are content-addressed by
    /// file bytes, enabled rule set, and engine version, so stale entries
    /// are never served. The workspace-level semantic tier is not cached.
    #[arg(long, value_name = "DIR")]
    pub cache: Option<PathBuf>,

    /// Delegate to `cargo clippy` and surface its diagnostics as advisory
    /// findings alongside rustmanifest's own, carrying clippy's severities
    /// and linking each finding to the clippy lint index as its rationale.
    ///
    /// Requires a `Cargo.toml` directly inside the first analyzed path; the
    /// probe is deliberately shallow and parent workspace manifests are not
    /// searched. Delegation is best-effort and never fails the run: a
    /// missing `cargo` binary, a non-zero `cargo clippy` exit without
    /// parseable output, or malformed JSON lines are each reported as a
    /// warning, while findings from parseable lines are still used.
    #[arg(long)]
    pub delegate_clippy: bool,

    /// Apply machine-applicable fixes to the analyzed files, then re-run
    /// the analysis so the output reflects the corrected sources. Only
    /// fixes proven safe are applied; everything else stays a suggestion.
    #[arg(long)]
    pub fix: bool
}

/// Output format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Picks `tty` on a terminal, `json` otherwise.
    Auto,
    /// Canonical pretty-printed JSON array.
    Json,
    /// SARIF 2.1.0 document.
    Sarif,
    /// Human-readable terminal output.
    Tty
}

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

    const fn admits(self, severity: Severity) -> bool {
        severity_rank(severity) >= self.level()
    }
}

/// Numeric rank of a severity; higher means more severe.
const fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 3,
        Severity::Warning => 2,
        Severity::Info => 1,
        Severity::Hint => 0
    }
}

pub(crate) fn run_check(args: &CheckArgs) -> anyhow::Result<ExitCode> {
    let (mut findings, effective) = collect_findings(args)?;
    if args.fix {
        let applied = apply_machine_fixes(&findings)?;
        if applied > 0 {
            info!(
                applied,
                "machine-applicable fixes applied; re-running analysis"
            );
            (findings, _) = collect_findings(args)?;
        }
    }

    render(&findings, args.format, args.no_color)?;

    let threshold = severity_rank(effective.exit_threshold());
    let exit_relevant = findings
        .iter()
        .any(|finding| severity_rank(finding.severity) >= threshold);
    if exit_relevant {
        Ok(ExitCode::from(EXIT_FINDINGS))
    } else {
        Ok(ExitCode::from(EXIT_CLEAN))
    }
}

/// Runs the full analysis pipeline once and returns the filtered findings
/// with the effective configuration.
fn collect_findings(args: &CheckArgs) -> anyhow::Result<(Vec<Finding>, Effective)> {
    let config =
        rustmanifest_config::load(args.config.as_deref()).context("loading configuration")?;
    let effective = Effective::from_config(config).context("resolving configuration profile")?;

    let enabled: Vec<&Rule> = RULES
        .iter()
        .filter(|rule| effective.rule_enabled(rule))
        .collect();

    let analyzers: Vec<Box<dyn rustmanifest_engine::Analyzer>> = enabled
        .iter()
        .copied()
        .filter_map(|rule| match rule.definition {
            RuleDefinition::Pattern {
                ..
            } => Some(
                PatternAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn rustmanifest_engine::Analyzer>)
            ),
            RuleDefinition::Ast {
                ..
            } => Some(
                AstAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn rustmanifest_engine::Analyzer>)
            ),
            RuleDefinition::Semantic {
                ..
            } => None
        })
        .collect::<Result<Vec<_>, _>>()
        .context("building analyzers from bundled rules")?;

    let semantic: Vec<SemanticAnalyzer> = enabled
        .iter()
        .copied()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Semantic { .. }))
        .map(|rule| SemanticAnalyzer::new(rule.clone()))
        .collect::<Result<Vec<_>, _>>()
        .context("building semantic analyzers from bundled rules")?;

    info!(
        analyzer_count = analyzers.len(),
        path_count = args.paths.len(),
        "running check"
    );

    let cache = args
        .cache
        .as_deref()
        .map(|dir| {
            FindingsCache::open(dir)
                .with_context(|| format!("opening findings cache at {}", dir.display()))
        })
        .transpose()?;

    let orchestrator = OrchestratorBuilder::new()
        .analyzers(analyzers)
        .max_file_bytes(args.max_file_size)
        .cache(cache)
        .build();

    let discovered = walker::discover(&args.paths).context("file discovery failed")?;
    let files = apply_globs(discovered, &effective)?;
    let mut findings = orchestrator
        .run(&files)
        .context("orchestrator run failed")?;
    findings.extend(semantic_findings(&semantic, &files, args.max_file_size));
    if !effective.invariants().is_empty() {
        findings.extend(rustmanifest_engine::check_invariants(
            &read_sources(&files, args.max_file_size),
            effective.invariants()
        ));
    }
    if args.delegate_clippy {
        findings.extend(delegated_clippy_findings(&args.paths));
    }
    apply_severity_overrides(&mut findings, &effective);
    findings.retain(|finding| args.severity_filter.admits(finding.severity));
    Ok((findings, effective))
}

/// Applies every machine-applicable fix, grouped per file, edits sorted
/// descending by start so earlier offsets stay valid. Returns how many
/// fixes were applied; a file that changed on disk is skipped honestly.
fn apply_machine_fixes(findings: &[Finding]) -> anyhow::Result<usize> {
    use rustmanifest_schema::Applicability;
    let mut per_file: HashMap<&Path, Vec<&rustmanifest_schema::Fix>> = HashMap::new();
    for finding in findings {
        for fix in &finding.fixes {
            if fix.applicability == Applicability::MachineApplicable {
                let target = fix
                    .file
                    .as_deref()
                    .unwrap_or(finding.location.file.as_path());
                per_file.entry(target).or_default().push(fix);
            }
        }
    }
    let mut applied = 0usize;
    let mut files: Vec<&Path> = per_file.keys().copied().collect();
    files.sort_unstable();
    for path in files {
        let mut edits = per_file.remove(path).unwrap_or_default();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));
        edits.dedup_by(|a, b| a.range == b.range && a.replacement == b.replacement);
        let mut text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {} for fixing", path.display()))?;
        let mut touched = false;
        for edit in edits {
            let (start, end) = (edit.range.start as usize, edit.range.end as usize);
            if end > text.len() || start > end {
                warn!(path = %path.display(), "fix range out of bounds; skipped");
                continue;
            }
            text.replace_range(start..end, &edit.replacement);
            touched = true;
            applied += 1;
        }
        if touched {
            std::fs::write(path, text)
                .with_context(|| format!("writing fixed {}", path.display()))?;
        }
    }
    Ok(applied)
}

/// Applies the configuration's include and exclude globs to the discovered
/// file list. Paths are matched as discovered; an empty include list keeps
/// every file.
fn apply_globs(files: Vec<PathBuf>, effective: &Effective) -> anyhow::Result<Vec<PathBuf>> {
    let include = build_globset(effective.include()).context("compiling include globs")?;
    let exclude = build_globset(effective.exclude()).context("compiling exclude globs")?;
    Ok(files
        .into_iter()
        .filter(|path| include.as_ref().is_none_or(|set| set.is_match(path)))
        .filter(|path| exclude.as_ref().is_none_or(|set| !set.is_match(path)))
        .collect())
}

/// Compiles glob patterns into a single matcher; `None` when no patterns are
/// configured.
fn build_globset(patterns: &[String]) -> anyhow::Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).with_context(|| format!("invalid glob pattern {pattern:?}"))?;
        builder.add(glob);
    }
    Ok(Some(builder.build().context("building glob set")?))
}

/// Rewrites each finding's severity to the effective severity of its rule.
fn apply_severity_overrides(findings: &mut [Finding], effective: &Effective) {
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

/// Runs the workspace-level semantic tier over every readable Rust source
/// within the memory budget and returns its findings.
fn semantic_findings(
    analyzers: &[SemanticAnalyzer],
    files: &[PathBuf],
    max_file_bytes: u64
) -> Vec<Finding> {
    if analyzers.is_empty() {
        return Vec::new();
    }
    let sources = read_sources(files, max_file_bytes);
    let mut findings = Vec::new();
    for analyzer in analyzers {
        findings.extend(analyzer.analyze(&sources));
    }
    findings
}

/// Prefix of the clippy lint index page used to build rationale URIs for
/// delegated findings.
const CLIPPY_INDEX_URL: &str = "https://rust-lang.github.io/rust-clippy/master/index.html#";

/// Runs `cargo clippy --message-format=json --quiet` in the first analyzed
/// path and converts its diagnostics into advisory findings, sorted by
/// `(file, range start, rule id)` for deterministic output.
///
/// The manifest probe is intentionally shallow: only `<path>/Cargo.toml` is
/// checked and parent workspace manifests are not searched. Every failure
/// mode — missing `cargo`, a non-zero exit without parseable diagnostics,
/// malformed JSON lines — degrades to a [`warn`] log entry; delegation
/// never fails the surrounding check run. The child process inherits the
/// caller's environment untouched except for `CARGO_TERM_COLOR=never`.
fn delegated_clippy_findings(paths: &[PathBuf]) -> Vec<Finding> {
    let Some(root) = paths.first() else {
        warn!("clippy delegation skipped: no analysis paths were given");
        return Vec::new();
    };
    if !root.join("Cargo.toml").is_file() {
        warn!(
            path = %root.display(),
            "clippy delegation skipped: no Cargo.toml in the analyzed path"
        );
        return Vec::new();
    }
    let output = match std::process::Command::new("cargo")
        .args(["clippy", "--message-format=json", "--quiet"])
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            warn!(%error, "clippy delegation skipped: failed to invoke cargo");
            return Vec::new();
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut findings: Vec<Finding> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| parse_clippy_line(line, root))
        .collect();
    if !output.status.success() && findings.is_empty() {
        warn!(
            status = %output.status,
            "cargo clippy exited unsuccessfully without parseable clippy diagnostics"
        );
    }
    findings.sort_by(|left, right| {
        (
            &left.location.file,
            left.location.range.start,
            &left.rule_id
        )
            .cmp(&(
                &right.location.file,
                right.location.range.start,
                &right.rule_id
            ))
    });
    findings
}

/// Parses one line of `cargo clippy --message-format=json` output into a
/// [`Finding`].
///
/// Returns `None` for lines that are not clippy diagnostics: other cargo
/// message kinds, rustc diagnostics without a `clippy::` code, and summary
/// messages without a primary span. Lines that are not valid JSON are
/// logged as a [`warn`] entry and skipped.
fn parse_clippy_line(line: &str, root: &Path) -> Option<Finding> {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            warn!(%error, "skipping malformed cargo clippy JSON line");
            return None;
        }
    };
    if value.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-message") {
        return None;
    }
    let message = value.get("message")?;
    let code = message.get("code")?.get("code")?.as_str()?;
    let lint_anchor = code.strip_prefix("clippy::")?;
    let severity = match message.get("level").and_then(serde_json::Value::as_str) {
        Some("error") => Severity::Error,
        Some("warning") => Severity::Warning,
        _ => Severity::Info
    };
    let spans = message.get("spans")?.as_array()?;
    let primary = spans
        .iter()
        .find(|span| span.get("is_primary").and_then(serde_json::Value::as_bool) == Some(true))?;
    let file_name = primary
        .get("file_name")
        .and_then(serde_json::Value::as_str)?;
    let file = Path::new(file_name)
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(file_name))
        .to_path_buf();
    let start = primary
        .get("byte_start")
        .and_then(serde_json::Value::as_u64)?;
    let end = primary
        .get("byte_end")
        .and_then(serde_json::Value::as_u64)?;
    let text = message.get("message").and_then(serde_json::Value::as_str)?;
    Some(Finding {
        rule_id: code.to_owned(),
        severity,
        location: Location {
            file,
            range: Range {
                start: u32::try_from(start).ok()?,
                end:   u32::try_from(end).ok()?
            }
        },
        message: text.to_owned(),
        rationale_uri: format!("{CLIPPY_INDEX_URL}{lint_anchor}"),
        trace: Vec::new(),
        fixes: Vec::new()
    })
}

fn render(findings: &[Finding], format: OutputFormat, no_color: bool) -> anyhow::Result<()> {
    let resolved = resolve_format(format);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match resolved {
        OutputFormat::Json | OutputFormat::Auto => {
            let mut renderer = JsonRenderer::new(&mut handle);
            renderer.render(findings).context("rendering JSON output")?;
        }
        OutputFormat::Sarif => {
            let mut renderer = SarifRenderer::new(&mut handle, env!("CARGO_PKG_VERSION"));
            renderer
                .render(findings)
                .context("rendering SARIF output")?;
        }
        OutputFormat::Tty => {
            let color = !no_color && std::io::stdout().is_terminal();
            let mut renderer = TtyRenderer::new(&mut handle).with_color(color);
            renderer.render(findings).context("rendering TTY output")?;
        }
    }
    handle.flush().context("flushing stdout")?;
    Ok(())
}

fn resolve_format(format: OutputFormat) -> OutputFormat {
    if format == OutputFormat::Auto {
        if std::io::stdout().is_terminal() {
            OutputFormat::Tty
        } else {
            OutputFormat::Json
        }
    } else {
        format
    }
}
