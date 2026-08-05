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
use rustmanifest_engine::walker;

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
    pub active: bool,

    /// Additionally print each function's binarized feature vector — the
    /// fixed-order named binary features the learning layer trains on.
    #[arg(long)]
    pub features: bool,

    /// SCIP index file (`rust-analyzer scip .`) overlaying name-resolved
    /// call edges onto the graph before the transitive-effect closure.
    #[arg(long, value_name = "INDEX")]
    pub scip: Option<PathBuf>,

    /// Resolution overlay written by `rustmanifest-resolve`, so a call
    /// into a library states the type it hands back instead of a refusal.
    #[arg(long)]
    pub resolution: Option<PathBuf>,

    /// Expand every discovered crate via `cargo +nightly rustc -- \
    /// -Zunpretty=expanded` (all features, falling back to default);
    /// a successfully expanded crate's stream replaces its `src`
    /// files, so macro-generated functions become facts. The expanded
    /// universe is the compiled library: `#[cfg(test)]` modules are
    /// absent from it. Crates that fail to expand keep their plain
    /// sources.
    #[arg(long)]
    pub expand: bool
}

/// Runs the `facts` subcommand: discovers files, extracts behavioral
/// facts, and prints them as a JSON array.
pub(crate) fn run_facts(args: &FactsArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let mut sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    if args.expand {
        let summary = crate::expand::expand_paths(&args.paths);
        sources = merge_expansions(sources, summary.streams);
    }
    let scip = args
        .scip
        .as_deref()
        .map(rustmanifest_engine::ScipEdges::from_file)
        .transpose()
        .context("loading SCIP index")?;
    let resolution = crate::resolution(args.resolution.as_deref())?;
    let rendered =
        serde_json::to_string_pretty(&rustmanifest_engine::facts_export::facts_json_with(
            &sources,
            resolution.as_ref(),
            args.filter.as_deref(),
            args.active,
            args.features,
            scip.as_ref()
        ))
        .context("serializing facts")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").context("writing facts output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Replaces each expanded crate's `src` sources with its expanded
/// stream, presented as the crate's synthetic root module so crate and
/// module attribution keep working. Sources outside `src` — tests,
/// benches, examples, build scripts — stay as read, since the library
/// expansion does not cover them.
fn merge_expansions(
    sources: Vec<rustmanifest_engine::Source>,
    expansions: Vec<(PathBuf, String)>
) -> Vec<rustmanifest_engine::Source> {
    let mut merged: Vec<rustmanifest_engine::Source> = sources
        .into_iter()
        .filter(|source| {
            !expansions
                .iter()
                .any(|(dir, _)| source.path().starts_with(dir.join("src")))
        })
        .collect();
    for (dir, stream) in expansions {
        merged.push(rustmanifest_engine::Source::new(
            dir.join("src/lib.rs"),
            stream
        ));
    }
    merged
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

    use rustmanifest_engine::Source;

    use super::merge_expansions;

    #[test]
    fn expanded_streams_replace_their_crate_sources_only() {
        let sources = vec![
            Source::new(PathBuf::from("app/src/lib.rs"), "fn a() {}".to_owned()),
            Source::new(PathBuf::from("app/tests/it.rs"), "fn t() {}".to_owned()),
            Source::new(PathBuf::from("other/src/lib.rs"), "fn b() {}".to_owned()),
        ];
        let merged = merge_expansions(
            sources,
            vec![(PathBuf::from("app"), "fn a() {}\nfn gen() {}".to_owned())]
        );
        let paths: Vec<String> = merged
            .iter()
            .map(|source| source.path().display().to_string())
            .collect();
        assert!(paths.contains(&"app/tests/it.rs".to_owned()));
        assert!(paths.contains(&"other/src/lib.rs".to_owned()));
        assert_eq!(
            merged
                .iter()
                .filter(|source| source.path().starts_with("app/src"))
                .count(),
            1
        );
        assert!(merged.iter().any(|source| source.text().contains("fn gen")));
    }
}
