// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Acceptance of the recovered route table, measured against the table a
//! project declares for `OpenAPI` generation.
//!
//! The engine recovers routes by walking the router chain. A project that
//! generates an `OpenAPI` document also writes each endpoint by hand beside
//! its handler. Two tables, two independent means, one source: comparing
//! them measures the recovery instead of asserting it.
//!
//! The measurement is per repository and reports precision and recall
//! separately, with the disagreeing endpoints named. A repository that
//! declares nothing is reported as having no ground truth rather than as a
//! perfect or a failing score — a measurement against an absent reference
//! is not a measurement.

mod declared;
mod score;

use std::{collections::BTreeSet, path::Path};

use rustmanifest_engine::{Source, orchestrator::DEFAULT_MAX_FILE_BYTES, surface, walker};
use serde::Serialize;

pub use self::{
    declared::Endpoint,
    score::{Counts, Disagreement, Score}
};
use crate::{corpus::Corpus, error::EvalError};

/// The acceptance measurement over a whole corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouteAcceptance {
    /// Engine version the measurement was taken with.
    pub engine_version: String,
    /// One entry per corpus repository, in name order.
    pub repos:          Vec<RepoRoutes>
}

/// The measurement for one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct RepoRoutes {
    /// Corpus entry name.
    pub name:  String,
    /// How the recovered table compares against the declared one, or
    /// `None` when the repository declares no endpoints at all.
    pub score: Option<Score>
}

/// Measures every corpus entry under `corpus_root`.
///
/// # Errors
///
/// Returns [`EvalError::MissingCheckout`] when a pinned checkout does not
/// exist and [`EvalError::Engine`] when file discovery fails.
pub fn measure(corpus: &Corpus, corpus_root: &Path) -> Result<RouteAcceptance, EvalError> {
    let mut repos = Vec::with_capacity(corpus.repos.len());
    for entry in &corpus.repos {
        let path = corpus_root.join(&entry.path);
        if !path.is_dir() {
            return Err(EvalError::MissingCheckout {
                name: entry.name.clone(),
                path
            });
        }
        repos.push(RepoRoutes {
            name:  entry.name.clone(),
            score: measure_one(&path)?
        });
    }
    repos.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(RouteAcceptance {
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        repos
    })
}

/// Measures one checkout, answering `None` when it declares no endpoints.
fn measure_one(path: &Path) -> Result<Option<Score>, EvalError> {
    let sources = read_sources(path)?;
    let declared = declared::endpoints(&sources);
    if declared.is_empty() {
        return Ok(None);
    }
    let recovered: BTreeSet<Endpoint> = surface(&sources)
        .routes
        .iter()
        .map(|route| (route.method.to_ascii_uppercase(), route.path.clone()))
        .collect();
    Ok(Some(Score::of(&declared, &recovered)))
}

/// Loads every Rust source of `path` that fits the per-file memory budget.
fn read_sources(path: &Path) -> Result<Vec<Source>, EvalError> {
    let files = walker::discover(std::slice::from_ref(&path.to_path_buf()))?;
    Ok(files
        .into_iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "rs"))
        .filter(|file| {
            std::fs::metadata(file).is_ok_and(|meta| meta.len() <= DEFAULT_MAX_FILE_BYTES)
        })
        .filter_map(|file| {
            std::fs::read_to_string(&file)
                .ok()
                .map(|text| Source::new(file, text))
        })
        .collect())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn a_checkout_declaring_nothing_has_no_ground_truth() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("lib.rs"), b"pub fn add() {}").unwrap();
        assert_eq!(measure_one(dir.path()).unwrap(), None);
    }

    #[test]
    fn a_declared_endpoint_no_router_registers_is_a_miss() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("lib.rs"),
            b"#[utoipa::path(get, path = \"/a\")]\npub fn handler() {}"
        )
        .unwrap();
        let score = measure_one(dir.path()).unwrap().unwrap();
        assert_eq!(score.counts.declared, 1);
        assert_eq!(score.counts.agreed, 0);
        assert_eq!(score.disagreement.unrecovered, vec!["GET /a".to_owned()]);
    }

    #[test]
    fn a_missing_checkout_is_an_error_not_an_empty_score() {
        let corpus = Corpus {
            repos: vec![crate::corpus::RepoEntry {
                name:  "gone".to_owned(),
                path:  "nowhere".to_owned(),
                rev:   "0".to_owned(),
                notes: String::new()
            }]
        };
        let dir = tempdir().unwrap();
        assert!(measure(&corpus, dir.path()).is_err());
    }
}
