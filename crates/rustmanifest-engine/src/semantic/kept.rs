// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The question layer's third form: why does the program keep this?
//!
//! The answer assembles the keeping tier's own proofs into the chain
//! genre of the first two forms: the declaration that makes the store a
//! store, the verbs read from what each written method does to that
//! very field — each with the site that proves it — the routes serving
//! the records, and the writes that tell someone about them. Nothing
//! is read from a name for meaning, and the same honesty rules hold: a
//! store the facts cannot answer for is refused with every candidate
//! named by its declaring place.

use rustmanifest_schema::TraceStep;

use super::{
    keeping::{keeping, servings, stores, telling},
    why::Answer,
    workspace::Workspace
};

/// Answers why the program keeps the store `asked` names.
///
/// A store is asked for by its `Owner.field` key, its owner, its field,
/// or its record type — whichever the asker holds.
#[must_use]
pub fn why_kept(workspace: &Workspace<'_>, asked: &str) -> Answer {
    let held = stores(workspace);
    let matched: Vec<usize> = held
        .iter()
        .enumerate()
        .filter(|(_, store)| {
            format!("{}.{}", store.owner, store.field) == asked
                || store.owner == asked
                || store.field == asked
                || store.record == asked
        })
        .map(|(index, _)| index)
        .collect();
    let [index] = matched[..] else {
        return unmatched(&held, asked, &matched);
    };
    let store = &held[index];
    let key = format!("{}.{}", store.owner, store.field);
    let mut steps = vec![TraceStep {
        fact:   format!(
            "`{key}` is declared as a `{}` of `{}` records{}",
            store.shape,
            store.record,
            store
                .key
                .as_ref()
                .map_or_else(String::new, |looked| format!(", looked into by `{looked}`"))
        ),
        reason: "a store is a field whose declared type is a collection of a type the workspace \
                 declares"
            .to_owned()
    }];
    if store.wire {
        steps.push(TraceStep {
            fact:   format!("`{}` declares itself a wire shape", store.owner),
            reason: "a serialization derive written on the type says its values are made to \
                     leave or enter the process"
                .to_owned()
        });
    }
    let (verbs, routes, told) = proofs(workspace, &key, &mut steps);
    let conclusion = if verbs == 0 && routes == 0 && told == 0 {
        format!(
            "`{key}` keeps `{}` records by declaration alone — no written method touches it",
            store.record
        )
    } else {
        format!(
            "the program keeps `{}` records in `{key}`: {verbs} written method(s), {routes} \
             route(s), {told} telling write(s) prove it",
            store.record
        )
    };
    Answer {
        refused: false,
        conclusion,
        steps
    }
}

/// Appends the written proofs of one store to `steps`: the verbs read
/// from methods, the routes serving the records, and the writes that
/// tell someone about them — counted apart so the conclusion can say
/// which kind carried the answer.
fn proofs(
    workspace: &Workspace<'_>,
    key: &str,
    steps: &mut Vec<TraceStep>
) -> (usize, usize, usize) {
    let mut verbs = 0usize;
    for entry in keeping(workspace) {
        if entry.store != key {
            continue;
        }
        verbs += 1;
        steps.push(TraceStep {
            fact:   format!(
                "fn `{}` {} — proven at bytes {}..{}",
                entry.function, entry.verb, entry.site.start, entry.site.end
            ),
            reason: "the verb is read from what the method does to that very field, not from \
                     its name"
                .to_owned()
        });
    }
    let mut routes = 0usize;
    for serving in servings(workspace) {
        if serving.store != key {
            continue;
        }
        routes += 1;
        steps.push(TraceStep {
            fact:   format!(
                "route `{} {}` reaches `{}`, which {}",
                serving.method, serving.path, serving.reaches, serving.verb
            ),
            reason: "the route table names the handler, and the handler's own body proves the \
                     verb"
                .to_owned()
        });
    }
    let mut told = 0usize;
    for shown in telling(workspace) {
        if shown.store != key {
            continue;
        }
        told += 1;
        steps.push(TraceStep {
            fact:   format!(
                "fn `{}` writes {} out at bytes {}..{}",
                shown.function, shown.position, shown.site.start, shown.site.end
            ),
            reason: "the written value's composed origin names the store's own field".to_owned()
        });
    }
    (verbs, routes, told)
}

/// The refusal for a name no store answers for: what was looked for,
/// and every candidate by its own key — an ambiguous asker can ask
/// again more precisely.
fn unmatched(held: &[super::keeping::Store], asked: &str, matched: &[usize]) -> Answer {
    if matched.is_empty() {
        return Answer {
            refused:    true,
            conclusion: format!("no store named `{asked}` is recorded"),
            steps:      Vec::new()
        };
    }
    let listed: Vec<String> = matched
        .iter()
        .filter_map(|&index| held.get(index))
        .map(|store| {
            format!(
                "`{}.{}` keeping `{}`",
                store.owner, store.field, store.record
            )
        })
        .collect();
    Answer {
        refused:    true,
        conclusion: format!(
            "`{asked}` is ambiguous: {} stores — {}",
            matched.len(),
            listed.join(", ")
        ),
        steps:      Vec::new()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::why_kept;
    use crate::semantic::{
        behavior::behavior_with,
        settling::{Settling, settle_identity},
        workspace::Workspace
    };

    /// What a compiler settles about the fixture: the type holding
    /// `tasks`, what it carries, and which body each call reaches.
    fn settled_over(sources: &[crate::source::Source]) -> rustmanifest_schema::Resolution {
        let mut resolution = Settling::over("src/lib.rs", sources[0].text())
            .fields("self.tasks", "Store", "BTreeMap<u64, Task>")
            .rewound()
            .all_typed("self.tasks", "BTreeMap<u64, Task>")
            .rewound()
            .fields("task.title", "Task", "String")
            .done();
        let facts = behavior_with(sources, Some(&resolution));
        settle_identity(&mut resolution, sources, &facts);
        resolution
    }

    fn one(text: &str) -> Vec<crate::source::Source> {
        vec![crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )]
    }

    const KEPT: &str = "use std::collections::BTreeMap;\n\npub struct Task {\n    pub title: \
                        String,\n}\n\npub struct Store {\n    pub tasks: BTreeMap<u64, \
                        Task>,\n}\n\nimpl Store {\n    pub fn add(&mut self, title: String) {\n   \
                        \
                        let id = self.tasks.len() as u64;\n        self.tasks.insert(id, Task { \
                        title });\n    }\n\n    pub fn titles(&self) -> Vec<String> {\n        \
                        self.tasks.values().map(|task| task.title.clone()).collect()\n    }\n}\n";

    /// The chain carries the declaration and every proven verb with its
    /// byte-ranged site, and the conclusion counts the proofs.
    #[test]
    fn a_kept_store_answers_with_its_declaration_and_verbs() {
        let sources = one(KEPT);
        let held = why_kept(&Workspace::resolved(&sources, &settled_over(&sources)), "Store.tasks");
        assert!(held.conclusion.contains("`Task` records"), "{held:?}");
        assert!(
            held.steps[0].fact.contains("`BTreeMap` of `Task` records"),
            "{held:?}"
        );
        assert!(
            held.steps
                .iter()
                .any(|step| step.fact.contains("`Store::add` adds a record")
                    && step.fact.contains("bytes")),
            "{held:?}"
        );
        assert!(
            held.steps
                .iter()
                .any(|step| step.fact.contains("`Store::titles` reads the records")),
            "{held:?}"
        );
    }

    /// The record type finds its store: the asker holds any of the
    /// names, not only the key.
    #[test]
    fn the_record_type_finds_its_store() {
        let sources = one(KEPT);
        let held = why_kept(&Workspace::resolved(&sources, &settled_over(&sources)), "Task");
        assert!(held.conclusion.contains("`Store.tasks`"), "{held:?}");
    }

    /// A name no store answers for is refused by name.
    #[test]
    fn an_unknown_store_is_refused() {
        let sources = one(KEPT);
        let held = why_kept(&Workspace::resolved(&sources, &settled_over(&sources)), "Ghost");
        assert!(held.conclusion.contains("no store named"), "{held:?}");
    }

    /// An ambiguous name is refused with every candidate's key.
    #[test]
    fn an_ambiguous_name_is_refused_with_keys() {
        let text = "use std::collections::BTreeMap;\n\npub struct Task {\n    pub title: \
                    String,\n}\n\npub struct Live {\n    pub tasks: BTreeMap<u64, \
                    Task>,\n}\n\npub struct Done {\n    pub tasks: BTreeMap<u64, Task>,\n}\n";
        let sources = one(text);
        let held = why_kept(&Workspace::resolved(&sources, &settled_over(&sources)), "tasks");
        assert!(held.conclusion.contains("ambiguous"), "{held:?}");
        assert!(held.conclusion.contains("`Live.tasks`"), "{held:?}");
        assert!(held.conclusion.contains("`Done.tasks`"), "{held:?}");
    }
}
