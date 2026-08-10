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

use rustmanifest_engine::{Workspace, features, role};
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
pub fn learn_roles(workspace: &Workspace<'_>, options: LearnOptions) -> Vec<LearnedClause> {
    let facts = workspace.facts();
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

    use rustmanifest_engine::{Source, Workspace};

    use super::{LearnOptions, learn_roles};

    #[test]
    fn the_chain_carries_the_precedent_step_with_its_verdict() {
        let mut text = String::new();
        for index in 0..12 {
            writeln!(
                text,
                "pub struct Wide{index} {{ pub name: String, pub age: u8 }}\nimpl \
                 std::fmt::Display for Wide{index} {{ fn fmt(&self, out: &mut \
                 std::fmt::Formatter<'_>) -> std::fmt::Result {{ write!(out, \"name: {{}}, age: \
                 {{}}\", self.name, self.age) }} }}"
            )
            .unwrap();
        }
        for index in 0..10 {
            writeln!(
                text,
                "pub struct Slim{index} {{ pub name: String }}\nimpl std::fmt::Display for \
                 Slim{index} {{ fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> \
                 std::fmt::Result {{ write!(out, \"{{}}\", self.name) }} }}"
            )
            .unwrap();
        }
        text.push_str("pub struct User { pub name: String, pub age: u8 }\n");
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text)];
        let held = super::deduce_with_precedent(&sources, "User", "Display").unwrap();
        let step = held
            .chain
            .steps
            .iter()
            .find(|step| step.fact.contains("precedent"))
            .unwrap_or_else(|| panic!("{:#?}", held.chain.steps));
        assert!(step.fact.contains("labeled-list"), "{}", step.fact);
        assert!(step.fact.contains("agrees with"), "{}", step.fact);
        assert!(step.fact.contains("clause:"), "{}", step.fact);
        assert!(held.gain.is_some());
    }

    /// A printing precedent has no standing on a `Default` chain: the
    /// catalog's marker names a form outside the ranked ones, and the
    /// advisory step stays out.
    #[test]
    fn a_display_precedent_stays_silent_on_a_default_chain() {
        let mut text = String::new();
        for index in 0..12 {
            use std::fmt::Write;
            writeln!(
                text,
                "pub struct Wide{index} {{ pub name: String, pub age: u8 }}\nimpl \
                 std::fmt::Display for Wide{index} {{ fn fmt(&self, out: &mut \
                 std::fmt::Formatter<'_>) -> std::fmt::Result {{ write!(out, \"name: {{}}, age: \
                 {{}}\", self.name, self.age) }} }}"
            )
            .unwrap();
        }
        text.push_str("pub struct User { pub name: String, pub age: u8 }\n");
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text)];
        let held = super::deduce_with_precedent(&sources, "User", "Default").unwrap();
        assert!(held.gain.is_some());
        assert!(
            held.chain
                .steps
                .iter()
                .all(|step| !step.fact.contains("precedent")),
            "{:#?}",
            held.chain.steps
        );
    }

    #[test]
    fn precedent_above_the_gate_ranks_and_cites_its_clause() {
        let mut precedents = Vec::new();
        for _ in 0..12 {
            precedents.push(super::FormPrecedent {
                features: vec![1, 0],
                chose:    "labeled-list".to_owned()
            });
            precedents.push(super::FormPrecedent {
                features: vec![0, 1],
                chose:    "bare-values".to_owned()
            });
        }
        let ranked = super::rank_forms(
            &precedents,
            &["multi_field", "single_field"],
            &[1, 0],
            &["labeled-list", "bare-values"],
            LearnOptions::default()
        );
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].name, "labeled-list", "{ranked:?}");
        assert!(!ranked[0].grounds.is_empty());
    }

    #[test]
    fn precedent_below_the_gate_ranks_nothing() {
        let precedents = vec![
            super::FormPrecedent {
                features: vec![1, 0],
                chose:    "labeled-list".to_owned()
            };
            3
        ];
        let ranked = super::rank_forms(
            &precedents,
            &["multi_field", "single_field"],
            &[1, 0],
            &["labeled-list"],
            LearnOptions::default()
        );
        assert!(ranked.is_empty());
    }

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
        let learned = learn_roles(&Workspace::new(&corpus()), LearnOptions::default());
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
        let first = learn_roles(&Workspace::new(&corpus()), LearnOptions::default());
        let second = learn_roles(&Workspace::new(&corpus()), LearnOptions::default());
        assert_eq!(first, second);
    }

    #[test]
    fn empty_sources_learn_nothing() {
        assert!(learn_roles(&Workspace::new(&[]), LearnOptions::default()).is_empty());
    }

    #[test]
    fn tiny_corpora_learn_nothing() {
        let source = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn log(text: &str) { println!(\"{text}\"); }\nfn calc(value: u64) -> u64 { value * \
             2 }"
            .to_owned()
        )];
        assert!(learn_roles(&Workspace::new(&source), LearnOptions::default()).is_empty());
    }

    #[test]
    fn absent_roles_are_skipped() {
        let learned = learn_roles(&Workspace::new(&corpus()), LearnOptions::default());
        assert!(learned.iter().all(|clause| clause.role != "mutator"));
    }
}

/// One precedent for form ranking: an instance's binary features and
/// the form its author chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormPrecedent {
    /// Binary feature vector of the instance the choice was made for.
    pub features: Vec<u8>,
    /// Name of the chosen form.
    pub chose:    String
}

/// One ranked form: its score and the satisfied clause literals that
/// earned it, so a chain can cite the convention it followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedForm {
    /// Form name.
    pub name:    String,
    /// Satisfied positive clauses for `instance`, more is stronger.
    pub score:   usize,
    /// The literals of the strongest satisfied clause, as evidence.
    pub grounds: Vec<String>
}

/// Ranks candidate `forms` for `instance` by the workspace's own
/// precedent, one deterministic machine per form.
///
/// Forms whose precedent falls below the admission gate train nothing;
/// when no form passes, the answer is empty and the caller's catalog
/// default stands — a ranking below the gate is a guess, and guesses
/// are not rankings.
#[must_use]
pub fn rank_forms(
    precedents: &[FormPrecedent],
    feature_names: &[&'static str],
    instance: &[u8],
    forms: &[&str],
    options: LearnOptions
) -> Vec<RankedForm> {
    if precedents.len() < options.min_role_examples.max(1) {
        return Vec::new();
    }
    let vectors: Vec<Vec<u8>> = precedents
        .iter()
        .map(|held| held.features.clone())
        .collect();
    let mut ranked = Vec::new();
    for form in forms {
        let targets: Vec<u8> = precedents
            .iter()
            .map(|held| u8::from(held.chose == *form))
            .collect();
        let positives: usize = targets.iter().copied().map(usize::from).sum();
        if positives < options.min_role_examples.max(1) {
            continue;
        }
        let mut machine =
            TsetlinMachine::quick(options.clauses, feature_names.len(), options.threshold);
        machine.fit(&vectors, &targets, options.epochs, options.seed);
        let mut score = 0usize;
        let mut grounds: Vec<String> = Vec::new();
        for rule in machine.rules() {
            if rule.polarity <= 0 || (rule.included.is_empty() && rule.negated.is_empty()) {
                continue;
            }
            let satisfied = rule
                .included
                .iter()
                .all(|index| instance.get(*index).copied() == Some(1))
                && rule
                    .negated
                    .iter()
                    .all(|index| instance.get(*index).copied() == Some(0));
            if satisfied {
                score += 1;
                let mut literals = feature_names_of(feature_names, &rule.included);
                literals.extend(
                    feature_names_of(feature_names, &rule.negated)
                        .into_iter()
                        .map(|name| format!("not {name}"))
                );
                if literals.len() > grounds.len() {
                    grounds = literals;
                }
            }
        }
        if score > 0 {
            ranked.push(RankedForm {
                name: (*form).to_owned(),
                score,
                grounds
            });
        }
    }
    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.name.cmp(&right.name))
    });
    ranked
}

/// Maps feature indices back to their stable names, for rankings.
fn feature_names_of(names: &[&'static str], indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .filter_map(|index| names.get(*index))
        .map(|name| (*name).to_owned())
        .collect()
}

/// The stable feature names of one type instance, for form ranking.
const FORM_FEATURES: [&str; 2] = ["multi_field", "has_string_field"];

/// The binary features of one type node, in [`FORM_FEATURES`] order.
fn type_form_features(node: &rustmanifest_engine::TypeNode) -> Vec<u8> {
    vec![
        u8::from(node.fields.len() > 1),
        u8::from(node.fields.iter().any(|field| field.type_name == "String")),
    ]
}

/// The display-form precedents the workspace's own written `Display`
/// impls state: the features of each implementing type, and whether
/// its written template labels its holes or prints bare values.
fn display_precedents(workspace: &Workspace<'_>) -> Vec<FormPrecedent> {
    use rustmanifest_engine::semantic::{Payload, Segment};
    let types = workspace.types();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        let Some(owner) = fact.function.strip_suffix("::fmt") else {
            continue;
        };
        let Some(node) = types.type_named(owner) else {
            continue;
        };
        if !types.traits_implemented(owner).contains(&"Display") {
            continue;
        }
        let labeled = fact.output_writes.iter().any(|write| {
            matches!(&write.payload, Payload::Template(segments)
            if segments.iter().any(|segment| {
                matches!(segment, Segment::Text(text) if text.contains(':'))
            }))
        });
        found.push(FormPrecedent {
            features: type_form_features(node),
            chose:    if labeled {
                "labeled-list".to_owned()
            } else {
                "bare-values".to_owned()
            }
        });
    }
    found
}

/// The display forms the precedent ranks — the only forms it has any
/// standing to advise on.
const DISPLAY_FORMS: [&str; 2] = ["labeled-list", "bare-values"];

/// Runs the engine's deduction with the workspace's precedent advising
/// the chain.
///
/// Where enough precedent exists, the advisory ranking step lands
/// after the catalog's choice, found by the chain's own structural
/// marker rather than by parsing the step's prose: agreement is
/// recorded as agreement, contradiction as contradiction — the
/// catalog's choice stands either way, and the chain says which
/// convention the workspace itself would rank first, citing the clause
/// that earned the rank. A choice outside the ranked forms — the
/// field-by-field build of `Default` — gets no advice: a printing
/// precedent has no standing there.
#[must_use]
pub fn deduce_with_precedent(
    sources: &[rustmanifest_engine::Source],
    type_name: &str,
    trait_name: &str
) -> Option<rustmanifest_engine::semantic::Deduction> {
    let workspace = Workspace::new(sources);
    let types = workspace.types();
    let node = types.type_named(type_name)?;
    let file = node.source;
    let instance = type_form_features(node);
    let precedents = display_precedents(&workspace);
    let ranked = rank_forms(
        &precedents,
        &FORM_FEATURES,
        &instance,
        &DISPLAY_FORMS,
        LearnOptions::default()
    );
    let mut held =
        rustmanifest_engine::semantic::deduce_impl(sources, file, type_name, trait_name);
    let spoken_for = held
        .catalog
        .clone()
        .filter(|choice| DISPLAY_FORMS.contains(&choice.form.as_str()));
    if let (Some(choice), Some(top)) = (spoken_for, ranked.first()) {
        let agreement = top.name == choice.form;
        held.chain.steps.insert(choice.step + 1, rustmanifest_schema::TraceStep {
            fact:   format!(
                "the workspace's own precedent ({} impl(s)) ranks `{}` first and {} the catalog \
                 — clause: {}",
                precedents.len(),
                top.name,
                if agreement { "agrees with" } else { "contradicts" },
                top.grounds.join(" AND ")
            ),
            reason: "learned clauses describe the codebase; the catalog's choice stands"
                .to_owned()
        });
    }
    Some(held)
}
