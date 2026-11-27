// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `audit` subcommand: tier-3 orchestration of external Rust tooling.
//!
//! Discovers which of the standard verification tools are installed and
//! runs them, aggregating their output into one report. The manifest never
//! reimplements their logic: it delegates and normalises.

use std::{io::Write, process::ExitCode};

use anyhow::Context;
use clap::Parser;

use crate::EXIT_CLEAN;

/// Arguments for the `audit` subcommand.
#[derive(Debug, Parser)]
pub struct AuditArgs {
    /// Path to the workspace root (defaults to the current directory).
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,

    /// Profile: `pr` (fast) or `nightly` (Miri, sanitizers, fuzz, mutants).
    #[arg(long, default_value = "pr")]
    pub profile: String
}

/// A tool the orchestrator knows how to invoke.
#[derive(Clone)]
struct Tool {
    name:   &'static str,
    binary: &'static str,
    args:   &'static [&'static str]
}

const PR_TOOLS: &[Tool] = &[
    Tool {
        name:   "clippy",
        binary: "cargo",
        args:   &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings"
        ]
    },
    Tool {
        name:   "cargo-deny",
        binary: "cargo",
        args:   &["deny", "check"]
    },
    Tool {
        name:   "cargo-audit",
        binary: "cargo",
        args:   &["audit"]
    },
    Tool {
        name:   "cargo-udeps",
        binary: "cargo",
        args:   &["udeps", "--all-targets"]
    }
];

const NIGHTLY_TOOLS: &[Tool] = &[
    Tool {
        name:   "miri",
        binary: "cargo",
        args:   &["+nightly", "miri", "test", "--workspace"]
    },
    Tool {
        name:   "cargo-careful",
        binary: "cargo",
        args:   &["+nightly", "careful", "test", "--workspace"]
    },
    Tool {
        name:   "cargo-fuzz",
        binary: "cargo",
        args:   &["fuzz", "run", "--", "-max_total_time=60"]
    },
    Tool {
        name:   "cargo-semver-checks",
        binary: "cargo",
        args:   &["semver-checks", "check-release"]
    },
    Tool {
        name:   "cargo-mutants",
        binary: "cargo",
        args:   &["mutants", "--in-diff"]
    },
    Tool {
        name:   "cargo-hakari",
        binary: "cargo",
        args:   &["hakari", "check"]
    }
];

fn tool_available(binary: &str, args: &[&str]) -> bool {
    let mut command = std::process::Command::new(binary);
    for arg in args {
        command.arg(arg);
    }
    command.arg("--version");
    command.output().is_ok_and(|o| o.status.success())
}

/// Runs the `audit` subcommand.
pub(crate) fn run_audit(args: &AuditArgs) -> anyhow::Result<ExitCode> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "## audit ({})", args.profile).context("writing output")?;

    let tools = if args.profile == "nightly" {
        [PR_TOOLS, NIGHTLY_TOOLS].concat()
    } else {
        PR_TOOLS.to_vec()
    };

    for tool in &tools {
        write!(handle, "- {}: ", tool.name).context("writing output")?;
        if !tool_available(tool.binary, tool.args) {
            writeln!(handle, "SKIP (not installed)").context("writing output")?;
            continue;
        }
        let mut command = std::process::Command::new(tool.binary);
        command.current_dir(&args.path);
        for arg in tool.args {
            command.arg(arg);
        }
        let result = command.output();
        match result {
            Ok(output) => {
                if output.status.success() {
                    writeln!(handle, "OK").context("writing output")?;
                } else {
                    writeln!(handle, "FAIL").context("writing output")?;
                }
                if !output.stdout.is_empty() {
                    writeln!(handle, "  stdout:").context("writing output")?;
                    for line in String::from_utf8_lossy(&output.stdout).lines() {
                        writeln!(handle, "    {line}").context("writing output")?;
                    }
                }
                if !output.stderr.is_empty() {
                    writeln!(handle, "  stderr:").context("writing output")?;
                    for line in String::from_utf8_lossy(&output.stderr).lines() {
                        writeln!(handle, "    {line}").context("writing output")?;
                    }
                }
            }
            Err(error) => {
                writeln!(handle, "ERROR: {error}").context("writing output")?;
            }
        }
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
