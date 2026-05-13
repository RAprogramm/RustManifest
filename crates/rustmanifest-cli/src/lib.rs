// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Library half of the `rustmanifest` CLI binary.
//!
//! The binary in `src/main.rs` is a thin shim that delegates to [`run`],
//! which makes the command surface testable from integration tests without
//! spawning a child process.

use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
    process::ExitCode
};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use rustmanifest_engine::{OrchestratorBuilder, PatternAnalyzer, walker};
use rustmanifest_report::{JsonRenderer, Renderer, SarifRenderer, TtyRenderer};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, RuleDefinition, Severity};
use tracing::info;

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
    Check(CheckArgs)
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
    pub max_file_size: u64
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
        let severity_level: u8 = match severity {
            Severity::Error => 3,
            Severity::Warning => 2,
            Severity::Info => 1,
            Severity::Hint => 0
        };
        severity_level >= self.level()
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
        Command::Check(args) => run_check(&args)
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
    let analyzers: Vec<Box<dyn rustmanifest_engine::Analyzer>> = RULES
        .iter()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Pattern { .. }))
        .map(|rule| {
            PatternAnalyzer::new(rule.clone())
                .map(|analyzer| Box::new(analyzer) as Box<dyn rustmanifest_engine::Analyzer>)
        })
        .collect::<Result<Vec<_>, _>>()
        .context("building pattern analyzers from bundled rules")?;

    info!(
        analyzer_count = analyzers.len(),
        path_count = args.paths.len(),
        "running check"
    );

    let orchestrator = OrchestratorBuilder::new()
        .analyzers(analyzers)
        .max_file_bytes(args.max_file_size)
        .build();

    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let mut findings = orchestrator
        .run(&files)
        .context("orchestrator run failed")?;
    findings.retain(|finding| args.severity_filter.admits(finding.severity));

    render(&findings, args.format, args.no_color)?;

    if findings.is_empty() {
        Ok(ExitCode::from(EXIT_CLEAN))
    } else {
        Ok(ExitCode::from(EXIT_FINDINGS))
    }
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
