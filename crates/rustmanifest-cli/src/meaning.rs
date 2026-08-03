// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `meaning` subcommand: what the analyzed program does for an observer
//! outside the process.
//!
//! The default rendering is a table meant to be read: one block per
//! entry point, one aligned row per write — how certainly it happens,
//! which stream it goes to, the text it puts there, and the function
//! that does it. Properties of the standard library rather than of the
//! analyzed program are stated once as a footnote, not repeated on every
//! row. `--format json` emits the same claims as data, for tools.

use std::{fmt::Write as _, io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use rustmanifest_engine::{
    EntryKind, EntryMeaning, Grade, Multiplicity, OUTPUT_SINK_SET, OutputClaim, walker
};

use crate::EXIT_CLEAN;

/// Arguments for the `meaning` subcommand.
#[derive(Debug, Parser)]
pub struct MeaningArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = MeaningFormat::Text)]
    pub format: MeaningFormat
}

/// Rendering of the meaning report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MeaningFormat {
    /// Aligned table meant for a reader.
    Text,
    /// Canonical JSON array, one object per entry point.
    Json
}

/// Runs the `meaning` subcommand.
pub(crate) fn run_meaning(args: &MeaningArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let entries = rustmanifest_engine::program_meaning(&sources);
    let root = args.paths.first().cloned().unwrap_or_default();
    let rendered = match args.format {
        MeaningFormat::Text => text_report(&entries, &root),
        MeaningFormat::Json => serde_json::to_string_pretty(&json_report(&entries, &root))
            .context("serializing meaning report")?
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").context("writing meaning output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Renders the report as an aligned table with a single footnote.
fn text_report(entries: &[EntryMeaning], root: &std::path::Path) -> String {
    if entries.is_empty() {
        return "no process entry point found — this source slice declares no program".to_owned();
    }
    let mut out = String::new();
    let mut standard_stream_seen = false;
    for entry in entries {
        let _ = writeln!(
            out,
            "{} ({})",
            relative(&entry.file, root),
            entry_kind(entry.kind)
        );
        if entry.claims.is_empty() {
            for sentence in entry.sentences() {
                let _ = writeln!(out, "  {sentence}");
            }
            out.push('\n');
            continue;
        }
        let rows: Vec<[String; 4]> = entry.claims.iter().map(row).collect();
        let widths = column_widths(&rows);
        for (row, claim) in rows.iter().zip(&entry.claims) {
            standard_stream_seen = true;
            let _ = writeln!(
                out,
                "  {:<certainty$}  {:<stream$}  {:<payload$}  {}",
                row[0],
                row[1],
                row[2],
                row[3],
                certainty = widths[0],
                stream = widths[1],
                payload = widths[2]
            );
            if let Some(reason) = claim.weakening() {
                let _ = writeln!(
                    out,
                    "  {:<width$}  weakened by {reason}",
                    "",
                    width = widths[0]
                );
            }
        }
        out.push('\n');
    }
    if standard_stream_seen {
        out.push_str(
            "Writes to a standard stream are attempts: the library can divert them through its \
             capture hook, and a failed write panics instead of writing.\n"
        );
    }
    out.trim_end().to_owned()
}

/// The four columns of one row: certainty, stream, text, function.
fn row(claim: &OutputClaim) -> [String; 4] {
    let repetition = if claim.write.multiplicity == Multiplicity::Unbounded {
        " (per iteration)"
    } else {
        ""
    };
    [
        certainty(claim.execution).to_owned(),
        claim.write.stream.to_string(),
        format!("{}{repetition}", claim.payload_text()),
        claim.function.clone()
    ]
}

/// Width of every column but the last.
fn column_widths(rows: &[[String; 4]]) -> [usize; 3] {
    let mut widths = [0usize; 3];
    for row in rows {
        for (index, width) in widths.iter_mut().enumerate() {
            *width = (*width).max(row[index].chars().count());
        }
    }
    widths
}

/// How certainly a write happens, in one word.
const fn certainty(grade: Grade) -> &'static str {
    match grade {
        Grade::Certain => "always",
        Grade::Possible => "maybe",
        Grade::Dead => "never",
        Grade::Unreached => "unreached"
    }
}

/// How an entry point is declared, in one phrase.
const fn entry_kind(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Plain => "entry point",
        EntryKind::Runtime => "entry point behind a runtime attribute",
        EntryKind::Conditional => "entry point behind a configuration gate"
    }
}

/// Renders the report as data.
///
/// Every entry carries `sink-set`, the version of the recognizer list
/// this build ran, so an entry with no claim reads as a versioned
/// negative result rather than as an absence of writes.
fn json_report(entries: &[EntryMeaning], root: &std::path::Path) -> serde_json::Value {
    let documents: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "entry": relative(&entry.file, root),
                "entry-kind": entry_kind(entry.kind),
                "sink-set": OUTPUT_SINK_SET,
                "claims": entry
                    .claims
                    .iter()
                    .map(|claim| claim_json(claim, root))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    serde_json::Value::Array(documents)
}

/// Renders one claim as data, with every field the text form shows and
/// the byte range that proves it.
fn claim_json(claim: &OutputClaim, root: &std::path::Path) -> serde_json::Value {
    serde_json::json!({
        "function": claim.function,
        "file": relative(&claim.file, root),
        "stream": claim.write.stream.to_string(),
        "text": claim.payload_text(),
        "text-fully-known": claim.write.payload.fully_known().is_some(),
        "certainty": certainty(claim.execution),
        "reach": certainty(claim.reach),
        "repeats": claim.write.multiplicity == Multiplicity::Unbounded,
        "weakened-by": claim.weakening(),
        "site": { "start": claim.write.site.start, "end": claim.write.site.end },
        "sink-set": claim.sink_set
    })
}

/// Shortens a path against the analyzed root so the report stays
/// readable; a path outside the root is left as written.
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::Path;

    use rustmanifest_engine::{EntryKind, EntryMeaning, OUTPUT_SINK_SET};

    use super::{json_report, relative, text_report};

    fn silent_entry() -> EntryMeaning {
        EntryMeaning {
            file:   "src/main.rs".to_owned(),
            kind:   EntryKind::Plain,
            claims: Vec::new()
        }
    }

    #[test]
    fn an_entry_with_no_claim_names_the_recognizer_set_that_found_none() {
        let report = text_report(&[silent_entry()], Path::new("."));
        assert!(
            report.contains(&format!("recognizer set v{OUTPUT_SINK_SET}")),
            "negative result must state its recognizer set: {report}"
        );
    }

    #[test]
    fn an_entry_with_no_claim_carries_the_recognizer_set_as_data() {
        let report = json_report(&[silent_entry()], Path::new("."));
        assert_eq!(report[0]["claims"].as_array().unwrap().len(), 0);
        assert_eq!(report[0]["sink-set"], OUTPUT_SINK_SET);
    }

    #[test]
    fn paths_shorten_against_the_analyzed_root() {
        assert_eq!(
            relative("/work/demo/src/main.rs", Path::new("/work/demo")),
            "src/main.rs"
        );
        assert_eq!(relative("src/main.rs", Path::new(".")), "src/main.rs");
        assert_eq!(
            relative("/elsewhere/main.rs", Path::new("/work/demo")),
            "/elsewhere/main.rs"
        );
    }
}
