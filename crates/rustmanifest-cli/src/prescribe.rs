// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `prescribe` subcommand: the code the canonical form asks for, and
//! what the change buys.
//!
//! The report says what the program is; this says what it should be.
//! They are separate because they are read in different moments: one to
//! learn a program, one to change it. A prescription is only printed
//! where the engine holds every fact the replacement needs — the proven
//! text, the stream it goes to, the terminator, and the signature it
//! sits under. Where a fact is missing, nothing is proposed and the
//! missing fact is named.

use std::{fmt::Write as _, io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::{Grade, Multiplicity, Newline, OutputClaim, Source, Stream};

use crate::EXIT_CLEAN;

/// Arguments for the `prescribe` subcommand.
#[derive(Debug, Parser)]
pub struct PrescribeArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Print the full derivation behind every prescription: the counts
    /// before and after, and the facts each count rests on.
    #[arg(long)]
    pub why: bool
}

/// Runs the `prescribe` subcommand.
pub(crate) fn run_prescribe(args: &PrescribeArgs) -> anyhow::Result<ExitCode> {
    let files =
        rustmanifest_engine::walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let rendered = prescriptions(&sources, args.why);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write!(handle, "{rendered}").context("writing prescription output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Builds every prescription the recorded facts license.
fn prescriptions(sources: &[Source], why: bool) -> String {
    let entries = rustmanifest_engine::program_meaning(sources);
    if entries.is_empty() {
        return "- no process entry point\n".to_owned();
    }
    let mut out = String::new();
    for entry in &entries {
        for claim in &entry.claims {
            let Some(prescription) = direct_write(claim) else {
                let _ = writeln!(
                    out,
                    "- `{}`: no prescription — {}",
                    claim.function,
                    missing(claim)
                );
                continue;
            };
            let _ = writeln!(
                out,
                "- `{}`\n\n```rust\n{prescription}\n```\n",
                claim.function
            );
            if why {
                out.push_str(&derivation(claim, sources));
            }
        }
    }
    if out.is_empty() {
        out.push_str("- nothing to prescribe\n");
    }
    out
}

/// The replacement the canonical form asks for, built from the recorded
/// facts alone: the proven text, its terminator, the stream, and whether
/// the owning function can carry a failure out.
fn direct_write(claim: &OutputClaim) -> Option<String> {
    let text = claim.write.payload.fully_known()?;
    if claim.execution != Grade::Certain || claim.write.multiplicity != Multiplicity::Once {
        return None;
    }
    let handle = match claim.write.stream {
        Stream::Stdout => "stdout",
        Stream::Stderr => "stderr",
        Stream::CallerSupplied | Stream::Unknown => return None
    };
    let terminator = match claim.write.newline {
        Newline::Appended => "\\n",
        Newline::None => ""
    };
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!(
        "use std::io::{{Write, {handle}}};\n\nfn {}() -> std::io::Result<()> {{\n    \
         {handle}().lock().write_all(b\"{escaped}{terminator}\")\n}}",
        claim.function
    ))
}

/// Which fact the engine lacks for a prescription it did not make.
fn missing(claim: &OutputClaim) -> String {
    if claim.write.payload.fully_known().is_none() {
        return "the written text is not proven in full".to_owned();
    }
    if claim.execution != Grade::Certain {
        return "the write does not run on every path, so replacing it changes behaviour"
            .to_owned();
    }
    if claim.write.multiplicity != Multiplicity::Once {
        return "the write repeats, and a repeated write asks for a buffered writer rather than \
                a direct one"
            .to_owned();
    }
    "the stream this write reaches is not identified".to_owned()
}

/// The counts the change moves, each with the fact behind it.
fn derivation(claim: &OutputClaim, sources: &[Source]) -> String {
    let text = claim.write.payload.fully_known().unwrap_or_default();
    let bytes = text.len() + usize::from(claim.write.newline == Newline::Appended);
    let mut out = String::from("  before → after\n");
    let _ = writeln!(out, "  - format-argument constructions per write: 1 → 0");
    let _ = writeln!(out, "  - lock acquisitions per write: 1 → 1");
    let _ = writeln!(out, "  - bytes handed to the stream: {bytes} → {bytes}");
    let _ = writeln!(
        out,
        "  - a failed write: panic → returned error at `{}`",
        claim.function
    );
    for step in evidence(sources) {
        let _ = writeln!(out, "  - {step}");
    }
    out.push('\n');
    out
}

/// The library facts the counts rest on, quoted where the library states
/// them.
fn evidence(sources: &[Source]) -> Vec<String> {
    let _ = sources;
    rustmanifest_engine::library_output_facts()
        .into_iter()
        .map(|fact| format!("{} [{}:{}]", fact.snippet, fact.file.display(), fact.line))
        .collect()
}
