// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Interpretable learning layer over the engine's fact base.
//!
//! Trains one binary Tsetlin machine per behavioral role on the engine's
//! binarized feature vectors, using the deterministic role heuristic as
//! the bootstrap labeling, and extracts every positive-polarity clause as
//! a readable conjunction over named features. The output is a rule list
//! a reviewer can audit line by line — never a black box.
//!
//! Determinism: training runs sequentially from a fixed caller-supplied
//! seed, so identical sources always yield identical clauses. Learned
//! clauses describe the codebase; they are not a source of findings.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

use rustmanifest_engine::{Source, behavior, features, role};
use tsetlin_rs::TsetlinMachine;

/// Fixed role order used for training and output.
const ROLE_NAMES: [&str; 6] = [
    "io",
    "mutator",
    "constructor",
    "accessor",
    "pure",
    "unknown"
];

/// One learned conjunctive clause voting for a role.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LearnedClause {
    /// Role the clause votes for.
    pub role:     String,
    /// Feature names that must be present.
    pub includes: Vec<String>,
    /// Feature names that must be absent.
    pub excludes: Vec<String>
}

/// Training knobs, all deterministic.
#[derive(Debug, Clone, Copy)]
pub struct LearnOptions {
    /// Clauses per role machine.
    pub clauses:           usize,
    /// Training epochs.
    pub epochs:            usize,
    /// Vote threshold of each machine.
    pub threshold:         i32,
    /// Seed for every machine; same seed, same clauses.
    pub seed:              u64,
    /// Fewest functions worth learning from; below it nothing is
    /// learned — clauses fitted on a handful of examples memorize
    /// them instead of expressing a convention.
    pub min_functions:     usize,
    /// Fewest positive examples a role needs for its machine to train.
    pub min_role_examples: usize
}

impl Default for LearnOptions {
    fn default() -> Self {
        Self {
            clauses:           20,
            epochs:            30,
            threshold:         10,
            seed:              42,
            min_functions:     50,
            min_role_examples: 10
        }
    }
}

/// Learns per-role clauses from the behavioral facts of `sources`.
///
/// Roles with no positive example in the sources are skipped. Clauses
/// with no literal (always true) and exact duplicates are dropped; the
/// result is sorted by role order, then lexicographically.
#[must_use]
pub fn learn_roles(sources: &[Source], options: LearnOptions) -> Vec<LearnedClause> {
    let facts = behavior(sources);
    if facts.len() < options.min_functions.max(1) {
        return Vec::new();
    }
    let names: Vec<&'static str> = features(&facts[0])
        .iter()
        .map(|feature| feature.name)
        .collect();
    let vectors: Vec<Vec<u8>> = facts
        .iter()
        .map(|fact| {
            features(fact)
                .iter()
                .map(|feature| u8::from(feature.active))
                .collect()
        })
        .collect();
    let labels: Vec<String> = facts.iter().map(|fact| role(fact).to_string()).collect();
    let mut learned = Vec::new();
    for role_name in ROLE_NAMES {
        let targets: Vec<u8> = labels
            .iter()
            .map(|label| u8::from(label == role_name))
            .collect();
        let positives: usize = targets.iter().copied().map(usize::from).sum();
        if positives < options.min_role_examples.max(1) {
            continue;
        }
        let mut machine = TsetlinMachine::quick(options.clauses, names.len(), options.threshold);
        machine.fit(&vectors, &targets, options.epochs, options.seed);
        for rule in machine.rules() {
            if rule.polarity <= 0 || (rule.included.is_empty() && rule.negated.is_empty()) {
                continue;
            }
            learned.push(LearnedClause {
                role:     role_name.to_owned(),
                includes: feature_names(&names, &rule.included),
                excludes: feature_names(&names, &rule.negated)
            });
        }
    }
    let order = |clause: &LearnedClause| {
        ROLE_NAMES
            .iter()
            .position(|name| *name == clause.role)
            .unwrap_or(ROLE_NAMES.len())
    };
    learned.sort_by(|a, b| {
        order(a)
            .cmp(&order(b))
            .then_with(|| a.includes.cmp(&b.includes))
            .then_with(|| a.excludes.cmp(&b.excludes))
    });
    learned.dedup();
    learned
}

/// Maps feature indices back to their stable names.
fn feature_names(names: &[&'static str], indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .filter_map(|index| names.get(*index))
        .map(|name| (*name).to_owned())
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::{fmt::Write, path::PathBuf};

    use rustmanifest_engine::Source;

    use super::{LearnOptions, learn_roles};

    fn corpus() -> [Source; 1] {
        let mut text = String::new();
        for index in 0..25 {
            writeln!(
                text,
                "fn log_{index}(text: &str) {{ println!(\"{{text}}\"); }}"
            )
            .unwrap();
            writeln!(text, "fn calc_{index}(value: u64) -> u64 {{ value * 2 }}").unwrap();
        }
        [Source::new(PathBuf::from("src/lib.rs"), text)]
    }

    #[test]
    fn io_clauses_mention_the_console_effect() {
        let learned = learn_roles(&corpus(), LearnOptions::default());
        let io_clauses: Vec<_> = learned
            .iter()
            .filter(|clause| clause.role == "io")
            .collect();
        assert!(!io_clauses.is_empty());
        assert!(io_clauses.iter().any(|clause| {
            clause.includes.iter().any(|name| name == "effect-console")
                || clause.excludes.iter().any(|name| name == "params>=2")
        }));
    }

    #[test]
    fn training_is_deterministic() {
        let first = learn_roles(&corpus(), LearnOptions::default());
        let second = learn_roles(&corpus(), LearnOptions::default());
        assert_eq!(first, second);
    }

    #[test]
    fn empty_sources_learn_nothing() {
        assert!(learn_roles(&[], LearnOptions::default()).is_empty());
    }

    #[test]
    fn tiny_corpora_learn_nothing() {
        let source = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn log(text: &str) { println!(\"{text}\"); }\nfn calc(value: u64) -> u64 { value * \
             2 }"
            .to_owned()
        )];
        assert!(learn_roles(&source, LearnOptions::default()).is_empty());
    }

    #[test]
    fn absent_roles_are_skipped() {
        let learned = learn_roles(&corpus(), LearnOptions::default());
        assert!(learned.iter().all(|clause| clause.role != "mutator"));
    }
}
