// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Gathering the numbers the coverage report is rendered from.

use std::collections::BTreeSet;

use anyhow::Context;
use rustmanifest_engine::{Coverage, Source, coverage_with, walker};

use super::{dependencies::declared_dependencies, options::CoverageArgs};

/// Everything a coverage run measured: the sources that were read, the
/// crate names declared around them, and the engine's counters.
pub(super) struct Measurement {
    /// Sources read within the default per-file memory budget.
    pub(super) sources:  Vec<Source>,
    /// Crate names declared by the manifests around the analyzed paths.
    pub(super) declared: BTreeSet<String>,
    /// Counters the engine produced over those sources.
    pub(super) report:   Coverage
}

/// Discovers files under the requested paths, reads their sources within
/// the default per-file memory budget, and measures coverage, overlaying
/// name-resolved call edges when a SCIP index was given.
pub(super) fn measure(args: &CoverageArgs) -> anyhow::Result<Measurement> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let declared = declared_dependencies(&args.paths);
    let report = match args.scip.as_deref() {
        Some(index) => {
            let edges =
                rustmanifest_engine::ScipEdges::from_file(index).context("loading SCIP index")?;
            rustmanifest_engine::coverage_indexed(&sources, &declared, &edges)
        }
        None => coverage_with(&sources, &declared)
    };
    Ok(Measurement {
        sources,
        declared,
        report
    })
}
