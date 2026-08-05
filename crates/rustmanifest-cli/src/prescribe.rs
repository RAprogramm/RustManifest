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

mod anchor;
mod options;
mod replacement;
mod tally;

use std::{fmt::Write as _, io::Write, process::ExitCode};

use anyhow::Context;
use rustmanifest_engine::Source;

pub use self::options::PrescribeArgs;
use self::{
    anchor::anchored,
    replacement::{direct_write, missing},
    tally::derivation
};
use crate::EXIT_CLEAN;

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
    let entries =
        rustmanifest_engine::program_meaning(&rustmanifest_engine::Workspace::new(sources));
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
            out.push_str(&anchored(claim, sources, &prescription));
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
