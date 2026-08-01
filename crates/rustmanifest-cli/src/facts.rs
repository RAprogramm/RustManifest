// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `facts` subcommand: behavioral fact export.
//!
//! Discovers Rust sources exactly like `check`, runs the engine's
//! behavioral extraction, and prints one JSON object per function: which
//! fields it reads and writes, what it constructs, which error types it
//! can return, and which side-effect categories its body touches. The
//! output is a canonical pretty-printed JSON array, deterministic across
//! runs, meant both for human inspection and as the substrate for
//! invariant rules.

use std::{io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::{FunctionFacts, Source, behavior, walker};

use crate::EXIT_CLEAN;

/// Arguments for the `facts` subcommand.
#[derive(Debug, Parser)]
pub struct FactsArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Only print functions whose qualified name contains this substring.
    #[arg(long)]
    pub filter: Option<String>,

    /// Only print functions with at least one write, effect, or error
    /// type — skip pure readers and empty bodies.
    #[arg(long)]
    pub active: bool
}

/// Runs the `facts` subcommand: discovers files, extracts behavioral
/// facts, and prints them as a JSON array.
pub(crate) fn run_facts(args: &FactsArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let facts = behavior(&sources);
    let selected: Vec<&FunctionFacts> = facts
        .iter()
        .filter(|entry| {
            args.filter
                .as_ref()
                .is_none_or(|needle| entry.function.contains(needle.as_str()))
        })
        .filter(|entry| {
            !args.active
                || !entry.writes.is_empty()
                || !entry.effects.is_empty()
                || !entry.error_types.is_empty()
        })
        .collect();
    let rendered = serde_json::to_string_pretty(
        &selected
            .iter()
            .map(|entry| json_object(entry, &sources))
            .collect::<Vec<_>>()
    )
    .context("serializing facts")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").context("writing facts output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Renders one function's facts as a JSON object.
fn json_object(entry: &FunctionFacts, sources: &[Source]) -> serde_json::Value {
    let file = sources
        .get(entry.source)
        .map_or_else(String::new, |source| source.path().display().to_string());
    serde_json::json!({
        "function": entry.function,
        "file": file,
        "reads": entry.reads,
        "writes": entry.writes,
        "constructs": entry.constructs,
        "error-types": entry.error_types,
        "effects": entry.effects.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "params": entry.params.iter().map(|param| serde_json::json!({
            "name": param.name,
            "type": param.type_name,
            "mode": param.mode.to_string(),
            "uses": param.uses,
            "field-reads": param.field_reads
        })).collect::<Vec<_>>(),
        "clones": entry.clones,
        "allocations": entry.allocations
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use rustmanifest_engine::{Source, behavior};

    use super::json_object;

    #[test]
    fn json_object_carries_every_fact_category() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "struct Wallet { balance: i64 }\nimpl Wallet { fn credit(&mut self, amount: i64) -> \
             Result<(), String> { self.balance += amount; println!(\"credited\"); Ok(()) } }"
                .to_owned()
        )];
        let facts = behavior(&sources);
        let object = json_object(&facts[0], &sources);
        assert_eq!(object["function"], "Wallet::credit");
        assert_eq!(object["file"], "src/lib.rs");
        assert_eq!(object["writes"][0], "Wallet.balance");
        assert_eq!(object["error-types"][0], "String");
        assert_eq!(object["effects"][0], "console");
    }
}
