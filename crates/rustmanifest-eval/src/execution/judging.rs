// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Running the corpus judge and writing its verdicts out.
//!
//! The command reports and never fails on a rejection: a rejection is
//! evidence about the engine, and the exit code stays clean so the
//! numbers are measured rather than chosen.

use std::{io::Write, process::ExitCode};

use crate::{cli::JudgeArgs, corpus, error::EvalError, exit::EXIT_CLEAN};

/// Runs the judge over the corpus and writes the verdicts as JSON.
pub(super) fn run_judge(args: &JudgeArgs) -> Result<ExitCode, EvalError> {
    let corpus = corpus::load(&args.corpus)?;
    let judged = crate::judge::judge(&corpus, &args.corpus_root, args.limit)?;
    let text = super::to_pretty_json(&judged)?;
    std::fs::write(&args.out, text).map_err(|source| EvalError::WriteFile {
        path: args.out.clone(),
        source
    })?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for verdict in &judged.verdicts {
        writeln!(
            handle,
            "{}\t{}\t{}\t{}",
            verdict.repo,
            verdict.genre,
            verdict.subject,
            verdict.error.as_deref().unwrap_or("accepted")
        )?;
    }
    writeln!(
        handle,
        "accepted {}/{} (cap {} per repo and genre), verdicts written to {}",
        judged.accepted(),
        judged.verdicts.len(),
        judged.limit,
        args.out.display()
    )?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
