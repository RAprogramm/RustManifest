// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Library half of the `rustmanifest` CLI binary.
//!
//! The binary in `src/main.rs` is a thin shim that delegates to [`run`],
//! which makes the command surface testable from integration tests without
//! spawning a child process.

pub mod coverage;
pub mod expand;
pub mod facts;
pub mod graph;
pub mod manifests;

use std::{
    collections::HashMap,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
    process::ExitCode
};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use globset::{Glob, GlobSet, GlobSetBuilder};
use rustmanifest_config::Effective;
use rustmanifest_engine::{
    AstAnalyzer, FindingsCache, OrchestratorBuilder, PatternAnalyzer, SemanticAnalyzer, walker
};
use rustmanifest_report::{JsonRenderer, Renderer, SarifRenderer, TtyRenderer};
use rustmanifest_rules_core::{RULES, methodology};
use rustmanifest_schema::{Finding, Location, Range, Rule, RuleDefinition, Severity};
use tracing::{info, warn};

pub use crate::{
    coverage::CoverageArgs,
    graph::{DiagramFormat, GraphArgs, GraphKind}
};

/// Default exit code: no findings at or above the requested severity.
pub const EXIT_CLEAN: u8 = 0;
/// Findings present at or above the requested severity.
pub const EXIT_FINDINGS: u8 = 1;
/// Operational failure (IO, argument parsing, walker error).
pub const EXIT_ERROR: u8 = 2;

/// Top-level CLI arguments.
#[derive(Debug, Parser)]
#[command(
    name = "rustmanifest",
    version,
    about = "Production-grade Rust review engine — methodology-as-code"
)]
pub struct Cli {
    /// Increase verbosity. May be repeated (`-v`, `-vv`, `-vvv`).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Selected subcommand.
    #[command(subcommand)]
    pub command: Command
}

/// Available subcommands for the `rustmanifest` binary.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the analysis engine across one or more paths.
    Check(CheckArgs),
    /// Report how much of the analyzed sources the engine understands.
    Coverage(CoverageArgs),
    /// Print the rationale and methodology excerpt for a rule.
    Explain(ExplainArgs),
    /// Print behavioral facts of every function as JSON.
    Facts(facts::FactsArgs),
    /// Print an architecture diagram of the analyzed sources.
    Graph(GraphArgs)
}

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
    pub delegate_clippy: bool
}

/// Arguments for the `explain` subcommand.
#[derive(Debug, Parser)]
pub struct ExplainArgs {
    /// Rule identifier to explain, e.g. `RM-SEC-001`.
    pub rule_id: String
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

/// Entry point used by both the binary and integration tests.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] for unrecoverable operational failures.
pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    init_tracing(cli.verbose);
    match cli.command {
        Command::Check(args) => run_check(&args),
        Command::Coverage(args) => coverage::run_coverage(&args),
        Command::Explain(args) => run_explain(&args),
        Command::Facts(args) => facts::run_facts(&args),
        Command::Graph(args) => graph::run_graph(&args)
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE
    };
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

fn run_check(args: &CheckArgs) -> anyhow::Result<ExitCode> {
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

/// Prints the rationale for one rule: identity, severity, tier, and the
/// methodology excerpt referenced by its rationale URI.
fn run_explain(args: &ExplainArgs) -> anyhow::Result<ExitCode> {
    let Some(rule) = RULES.iter().find(|rule| rule.id == args.rule_id) else {
        let known: Vec<&str> = RULES.iter().map(|rule| rule.id.as_str()).collect();
        anyhow::bail!(
            "unknown rule id {:?}; known rules: {}",
            args.rule_id,
            known.join(", ")
        );
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

/// Reads every readable Rust source within the per-file memory budget into
/// memory; unreadable and oversized files are skipped silently.
fn read_sources(files: &[PathBuf], max_file_bytes: u64) -> Vec<rustmanifest_engine::Source> {
    files
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| metadata.len() <= max_file_bytes)
        })
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| rustmanifest_engine::Source::new(path.clone(), text))
        })
        .collect()
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
        trace: Vec::new()
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
