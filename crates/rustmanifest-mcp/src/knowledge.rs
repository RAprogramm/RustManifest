// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge tools backing: project overview, behavioral facts, and
//! learned conventions computed over a workspace path.
//!
//! Every function here is synchronous and deterministic; the server
//! dispatches them onto the blocking pool. All strings are English.

use std::path::{Path, PathBuf};

use rustmanifest_engine::{
    Analyzer, AstAnalyzer, EngineError, PatternAnalyzer, SemanticAnalyzer, Source, Workspace,
    facts_export,
    orchestrator::DEFAULT_MAX_FILE_BYTES,
    overview::{Convention, overview_markdown},
    pragma, walker
};
use rustmanifest_learn::{LearnOptions, learn_roles};

/// Renders the engine's Markdown knowledge report for `target`.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn overview(target: &Path, top: usize) -> Result<String, EngineError> {
    let sources = sources_for(target)?;
    let workspace = rustmanifest_engine::Workspace::new(&sources);
    let conventions: Vec<Convention> = learn_roles(&workspace, LearnOptions::default())
        .into_iter()
        .map(|clause| Convention {
            role:     clause.role,
            includes: clause.includes,
            excludes: clause.excludes
        })
        .collect();
    Ok(overview_markdown(&workspace, top, &conventions))
}

/// Returns behavioral facts of every function under `target` as a JSON
/// array, optionally filtered by a substring of the qualified name.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn facts(target: &Path, filter: Option<&str>) -> Result<serde_json::Value, EngineError> {
    let sources = sources_for(target)?;
    Ok(facts_export::facts_json(
        &sources, filter, false, false, None
    ))
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
    Ok(learn_roles(
        &rustmanifest_engine::Workspace::new(&sources),
        LearnOptions::default()
    )
    .into_iter()
    .map(|clause| ConventionClause {
        role:   clause.role,
        when:   clause.includes.join(" AND "),
        unless: clause.excludes.join(" AND ")
    })
    .collect())
}

/// One step of a deduction chain on the wire.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ChainStep {
    /// The recorded fact the step claims.
    pub fact:   String,
    /// The reason the fact counts as established.
    pub reason: String
}

/// One finished deduction chain on the wire.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct DeductionRecord {
    /// What the chain concludes; a stopped chain names its failing
    /// claim.
    pub conclusion: String,
    /// The reasoning, one recorded fact per step; the workspace's own
    /// precedent advises after the catalog's choice where the admission
    /// gate lets it speak.
    pub steps:      Vec<ChainStep>,
    /// Whether every step held and the re-reading confirmed the gain.
    pub gained:     bool,
    /// Whether an installed body can still end the path by panic;
    /// absent where nothing was installed.
    pub panics:     Option<bool>
}

/// Deduces the impl of `trait_name` for `type_name` over `target` and
/// answers with the chain itself.
///
/// The block is installed in memory and confirmed by re-reading; no
/// byte touches a disk. A chain that stops carries its steps up to the
/// failing claim — the refusal is as auditable as the success.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn deduction(
    target: &Path,
    type_name: &str,
    trait_name: &str
) -> Result<DeductionRecord, EngineError> {
    let sources = sources_for(target)?;
    let held = rustmanifest_learn::deduce_with_precedent(&sources, type_name, trait_name)
        .unwrap_or_else(|| {
            rustmanifest_engine::semantic::deduce_impl(&sources, 0, type_name, trait_name)
        });
    Ok(DeductionRecord {
        conclusion: held.chain.conclusion,
        steps:      held
            .chain
            .steps
            .into_iter()
            .map(|step| ChainStep {
                fact:   step.fact,
                reason: step.reason
            })
            .collect(),
        gained:     held.gain.is_some(),
        panics:     held.gain.map(|gain| gain.panics)
    })
}

/// One derived intent on the wire: the ability a written call asks
/// for, with the call standing as evidence.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct IntentRecord {
    /// The type the intent is about.
    pub type_name:  String,
    /// The ability the evidence asks for.
    pub trait_name: String,
    /// The function whose written call states the intent.
    pub asked_by:   String,
    /// The written call itself, as the body spells it.
    pub call:       String,
    /// The written call standing as evidence.
    pub evidence:   ChainStep
}

/// Derives every intent the written calls under `target` prove: a call
/// only an uninstalled ability can satisfy is the author's stated
/// intent, and each record cites the function that wrote it.
///
/// # Errors
///
/// Returns an [`EngineError`] when file discovery fails.
pub fn intents(target: &Path) -> Result<Vec<IntentRecord>, EngineError> {
    let sources = sources_for(target)?;
    Ok(rustmanifest_engine::semantic::derived_intents(&sources)
        .into_iter()
        .map(|intent| IntentRecord {
            type_name:  intent.type_name,
            trait_name: intent.trait_name,
            asked_by:   intent.asked_by,
            call:       intent.call,
            evidence:   ChainStep {
                fact:   intent.evidence.fact,
                reason: intent.evidence.reason
            }
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

/// One inline suppression on the wire, with whether the run used it.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct SuppressionRecord {
    /// File the pragma stands in.
    pub file:    String,
    /// 1-based line of the pragma.
    pub line:    usize,
    /// Rule the pragma suppresses.
    pub rule_id: String,
    /// The justifying reason the pragma carries.
    pub reason:  String,
    /// Whether the pragma suppressed at least one occurrence in this
    /// run — a `false` here is the unused-suppression smell itself.
    pub used:    bool
}

/// Lists every inline suppression under `target`.
///
/// Each record says whether a full run of the bundled rules used it:
/// the suppression protocol made visible — what is silenced, on what
/// ground, and which silences have gone stale.
///
/// # Errors
///
/// Returns an [`EngineError`] when an analyzer cannot be built or file
/// discovery fails.
pub fn suppressions(target: &Path) -> Result<Vec<SuppressionRecord>, EngineError> {
    let files = if target.is_dir() {
        walker::discover(&[target])?
    } else {
        vec![target.to_path_buf()]
    };
    let sources: Vec<Source> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|meta| meta.len() <= DEFAULT_MAX_FILE_BYTES)
        })
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| Source::new(path.clone(), text))
        })
        .collect();
    let mut tiered: Vec<Box<dyn Analyzer>> = Vec::new();
    let mut semantic: Vec<SemanticAnalyzer> = Vec::new();
    for rule in rustmanifest_rules_core::RULES.iter() {
        match rule.definition {
            rustmanifest_schema::RuleDefinition::Pattern {
                ..
            } => tiered.push(Box::new(PatternAnalyzer::new(rule.clone())?)),
            rustmanifest_schema::RuleDefinition::Ast {
                ..
            } => tiered.push(Box::new(AstAnalyzer::new(rule.clone())?)),
            rustmanifest_schema::RuleDefinition::Semantic {
                ..
            } => semantic.push(SemanticAnalyzer::new(rule.clone())?)
        }
    }
    for source in &sources {
        for analyzer in &tiered {
            if !analyzer.is_excluded(source.path()) {
                drop(analyzer.analyze(source));
            }
        }
    }
    drop(rustmanifest_engine::semantic_findings(
        &semantic,
        &Workspace::new(&sources)
    ));
    let mut records = Vec::new();
    for source in &sources {
        let used = source.used_pragma_lines();
        for held in pragma::collect(source.text()) {
            records.push(SuppressionRecord {
                file:    source.path().display().to_string(),
                line:    held.line_index + 1,
                rule_id: held.rule_id,
                reason:  held.reason,
                used:    used.contains(&held.line_index)
            });
        }
    }
    Ok(records)
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
        for index in 0..25 {
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
    fn suppressions_tell_the_used_from_the_stale() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("guarded.rs"),
            "fn main() {\n    // rustmanifest: allow(RM-SEC-001) reason=\"test fixture \
             intentionally hardcodes a credential\"\n    let password = \"hunter2\";\n    // \
             rustmanifest: allow(RM-SEC-002) reason=\"nothing here ever matched\"\n    let \
             greeting = \"hello\";\n    drop((password, greeting));\n}\n"
        )
        .unwrap();
        let held = super::suppressions(dir.path()).unwrap();
        assert_eq!(held.len(), 2, "{held:?}");
        let secret = held
            .iter()
            .find(|record| record.rule_id == "RM-SEC-001")
            .unwrap();
        assert!(secret.used, "{secret:?}");
        assert_eq!(secret.line, 2);
        let stale = held
            .iter()
            .find(|record| record.rule_id == "RM-SEC-002")
            .unwrap();
        assert!(!stale.used, "{stale:?}");
    }

    #[test]
    fn deduction_answers_with_the_chain_and_the_confirmed_gain() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("lib.rs"),
            "pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n"
        )
        .unwrap();
        let held = super::deduction(dir.path(), "User", "Display").unwrap();
        assert!(held.gained, "{held:?}");
        assert_eq!(held.panics, Some(false), "{held:?}");
        assert!(held.conclusion.contains("by deduction"), "{held:?}");
        assert!(
            held.steps
                .iter()
                .any(|step| step.fact.contains("`name: String`")),
            "{held:?}"
        );
    }

    #[test]
    fn a_stopped_deduction_reports_its_refusal() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("lib.rs"),
            "pub struct Batch {\n    pub held: Vec<String>,\n}\n"
        )
        .unwrap();
        let held = super::deduction(dir.path(), "Batch", "Display").unwrap();
        assert!(!held.gained, "{held:?}");
        assert_eq!(held.panics, None, "{held:?}");
        assert!(held.conclusion.contains("`held`"), "{held:?}");
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
