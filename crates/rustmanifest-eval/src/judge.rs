// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The corpus judge: generated edits applied to real corpus trees and
//! judged by the real build.
//!
//! For every corpus repository the judge copies the checkout into a
//! bed, lists the edits the engine confirms — canon installations and
//! whole module extractions — lands each one into the copy exactly as
//! an editor edit would land it, runs `cargo check` over the copy,
//! restores the tree, and records the verdict — accepted, or rejected
//! with the compiler's own first words. The real checkouts are never
//! touched. The command reports and never fails on a rejection: a
//! rejection is evidence about the engine, and the fixes it demands
//! are worth more than a green gate.

mod bed;
mod candidate;
mod installs;

use std::path::Path;

use serde::Serialize;

use crate::{corpus::Corpus, error::EvalError};

/// The genre of one judged edit.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Genre {
    /// A canon trait installation into one file.
    Install,
    /// A whole module extraction: a rewritten parent and a planted
    /// module file.
    Extract
}

impl std::fmt::Display for Genre {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Install => "install",
            Self::Extract => "extract"
        })
    }
}

/// One judged edit.
#[derive(Debug, Serialize)]
pub struct Verdict {
    /// Corpus entry name.
    pub repo:     String,
    /// The genre of the judged edit.
    pub genre:    Genre,
    /// The judged subject: the type and trait, or the moved function.
    pub subject:  String,
    /// Whether the real build accepted the landed edit.
    pub accepted: bool,
    /// The compiler's first error line where it refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error:    Option<String>
}

/// The whole measurement: every verdict, with the caps stated so a
/// truncated sweep cannot read as a full one.
#[derive(Debug, Serialize)]
pub struct Judgement {
    /// Candidate cap applied per repository and genre.
    pub limit:    usize,
    /// Every judged edit, in corpus order.
    pub verdicts: Vec<Verdict>
}

impl Judgement {
    /// Accepted verdict count.
    #[must_use]
    pub fn accepted(&self) -> usize {
        self.verdicts.iter().filter(|held| held.accepted).count()
    }
}

/// Judges up to `limit` edits per corpus repository and genre.
///
/// # Errors
///
/// Returns an [`EvalError`] for operational failures: a missing
/// checkout, an unreadable tree, a bed that cannot be built.
pub fn judge(corpus: &Corpus, corpus_root: &Path, limit: usize) -> Result<Judgement, EvalError> {
    let mut verdicts = Vec::new();
    for entry in &corpus.repos {
        let origin = corpus_root.join(&entry.path);
        if !origin.is_dir() {
            return Err(EvalError::MissingCheckout {
                name: entry.name.clone(),
                path: origin
            });
        }
        let bed = bed::Bed::of(&origin, &entry.name)?;
        let listed = [(Genre::Install, installs::listed(bed.root(), limit)?)];
        for (genre, candidates) in listed {
            for candidate in candidates {
                let outcome = bed.judged(&candidate)?;
                verdicts.push(Verdict {
                    repo: entry.name.clone(),
                    genre,
                    subject: candidate.subject,
                    accepted: outcome.is_none(),
                    error: outcome
                });
            }
        }
    }
    Ok(Judgement {
        limit,
        verdicts
    })
}
