// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge tools backing: project overview, behavioral facts, and
//! learned conventions computed over a workspace path.
//!
//! Every function here is synchronous and deterministic; the server
//! dispatches them onto the blocking pool. All strings are English.

use std::path::{Path, PathBuf};

use rustmanifest_engine::{
    EngineError, Source, facts_export,
    orchestrator::DEFAULT_MAX_FILE_BYTES,
    overview::{Convention, overview_markdown},
    walker
};
use rustmanifest_learn::{LearnOptions, learn_roles};

/// Renders the engine's Markdown knowledge report for `target`.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn overview(target: &Path, top: usize) -> Result<String, EngineError> {
    let sources = sources_for(target)?;
    let conventions: Vec<Convention> = learn_roles(&sources, LearnOptions::default())
        .into_iter()
        .map(|clause| Convention {
            role:     clause.role,
            includes: clause.includes,
            excludes: clause.excludes
        })
        .collect();
    Ok(overview_markdown(&sources, top, &conventions))
}

/// Returns behavioral facts of every function under `target` as a JSON
/// array, optionally filtered by a substring of the qualified name.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn facts(target: &Path, filter: Option<&str>) -> Result<serde_json::Value, EngineError> {
    let sources = sources_for(target)?;
    let all = rustmanifest_engine::behavior(&sources);
    let selected: Vec<serde_json::Value> = all
        .iter()
        .filter(|entry| filter.is_none_or(|needle| entry.function.contains(needle)))
        .map(|entry| facts_export::fact_json(entry, &sources, false))
        .collect();
    Ok(serde_json::Value::Array(selected))
}

/// One learned convention clause on the wire.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ConventionClause {
    /// Role the clause votes for.
    pub role:   String,
    /// Feature names that must be present.
    #[serde(rename = "if")]
    pub when:   String,
    /// Feature names that must be absent.
    pub unless: String
}

/// Learns per-role convention clauses over `target` and returns them as
/// renderable data.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn conventions(target: &Path) -> Result<Vec<ConventionClause>, EngineError> {
    let sources = sources_for(target)?;
    Ok(learn_roles(&sources, LearnOptions::default())
        .into_iter()
        .map(|clause| ConventionClause {
            role:   clause.role,
            when:   clause.includes.join(" AND "),
            unless: clause.excludes.join(" AND ")
        })
        .collect())
}

/// Expands `target` into readable Rust sources within the memory budget.
fn sources_for(target: &Path) -> Result<Vec<Source>, EngineError> {
    let files: Vec<PathBuf> = if target.is_dir() {
        walker::discover(&[target])?
    } else {
        vec![target.to_path_buf()]
    };
    Ok(files
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| metadata.len() <= DEFAULT_MAX_FILE_BYTES)
        })
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| Source::new(path.clone(), text))
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

    use tempfile::TempDir;

    use super::{conventions, facts, overview};

    fn tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        let mut text = String::new();
        for index in 0..20 {
            use std::fmt::Write;
            writeln!(
                text,
                "pub fn log_{index}(text: &str) {{ println!(\"{{text}}\"); }}\npub fn \
                 calc_{index}(value: u64) -> u64 {{ value * 2 }}"
            )
            .unwrap();
        }
        fs::write(dir.path().join("src/lib.rs"), text).unwrap();
        dir
    }

    #[test]
    fn overview_renders_every_section() {
        let report = overview(tree().path(), 10).unwrap();
        assert!(report.contains("# Project overview"));
        assert!(report.contains("## Behavioral roles"));
    }

    #[test]
    fn facts_filter_narrows_functions() {
        let all = facts(tree().path(), None).unwrap();
        let narrowed = facts(tree().path(), Some("log_1")).unwrap();
        assert!(all.as_array().unwrap().len() > narrowed.as_array().unwrap().len());
        assert!(!narrowed.as_array().unwrap().is_empty());
    }

    #[test]
    fn conventions_learn_io_clauses() {
        let clauses = conventions(tree().path()).unwrap();
        assert!(clauses.iter().any(|clause| clause.role == "io"));
    }
}
