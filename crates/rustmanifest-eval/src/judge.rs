// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The corpus judge: generated installations applied to real corpus
//! trees and judged by the real build.
//!
//! For every corpus repository the judge copies the checkout into a
//! bed, lists the types the montage confirms an installation for,
//! lands each installed text into the copy exactly as an editor edit
//! would land it, runs `cargo check` over the copy, restores the file,
//! and records the verdict — accepted, or rejected with the compiler's
//! own first words. The real checkouts are never touched. The command
//! reports and never fails on a rejection: a rejection is evidence
//! about the engine, and the fixes it demands are worth more than a
//! green gate.

mod bed;
mod candidates;

use std::path::Path;

use serde::Serialize;

use crate::{corpus::Corpus, error::EvalError};

/// One judged installation.
#[derive(Debug, Serialize)]
pub struct Verdict {
    /// Corpus entry name.
    pub repo:       String,
    /// The type the installation is for.
    pub type_name:  String,
    /// The installed trait.
    pub trait_name: String,
    /// Whether the real build accepted the landed text.
    pub accepted:   bool,
    /// The compiler's first error line where it refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error:      Option<String>
}

/// The whole measurement: every verdict, with the caps stated so a
/// truncated sweep cannot read as a full one.
#[derive(Debug, Serialize)]
pub struct Judgement {
    /// Candidate cap applied per repository.
    pub limit:    usize,
    /// Every judged installation, in corpus order.
    pub verdicts: Vec<Verdict>
}

impl Judgement {
    /// Accepted verdict count.
    #[must_use]
    pub fn accepted(&self) -> usize {
        self.verdicts.iter().filter(|held| held.accepted).count()
    }
}

/// Judges up to `limit` installations per corpus repository.
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
        for candidate in candidates::listed(bed.root(), limit)? {
            let outcome = bed.judged(&candidate)?;
            verdicts.push(Verdict {
                repo:       entry.name.clone(),
                type_name:  candidate.type_name,
                trait_name: candidate.trait_name,
                accepted:   outcome.is_none(),
                error:      outcome
            });
        }
    }
    Ok(Judgement {
        limit,
        verdicts
    })
}
