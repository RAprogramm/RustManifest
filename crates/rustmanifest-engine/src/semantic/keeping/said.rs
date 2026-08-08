// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program is for, in one sentence, assembled from the readings
//! that proved each part.
//!
//! Nothing is added between them: the sentence names the records a
//! program holds, what its own methods do to them, how a run asks for
//! each of those, and whether what is held survives the run. A part no
//! reading proved is simply absent, so a shorter sentence is a smaller
//! claim rather than a vaguer one.

use std::fmt::Write as _;

use super::{
    answering::returners, asking::commands_with, ending::endings, fed::feedings_with,
    outliving::outliving, receiving::receivers, served::servings_with, store::stores,
    telling::telling, verb::keeping
};
use crate::semantic::{surface::surface, workspace::Workspace};

/// What a program does with the records it holds, in one sentence per
/// store.
///
/// Empty where the workspace holds no records: silence is "no store
/// proven", never "this program is about nothing".
#[must_use]
pub fn purpose(workspace: &Workspace<'_>) -> Vec<String> {
    let sources = workspace.sources();
    let facts = workspace.facts();
    let qualifiers = crate::semantic::meaning::qualifiers_by_fact(sources, facts);
    let composed = crate::semantic::summary::composition_with(
        sources,
        workspace.resolution(),
        facts,
        workspace.calls(),
        &qualifiers
    );
    let held = stores(workspace);
    let kept = keeping(workspace);
    let carried = outliving(workspace);
    let asked = commands_with(workspace, &composed);
    let served = servings_with(workspace, &composed);
    let told = telling(workspace);
    let answered = returners(sources);
    let received = receivers(sources);
    let fed = feedings_with(workspace, &composed);
    let routes = surface(sources).routes;
    let ends = ending_clause(workspace);
    let mut said = Vec::new();
    for store in &held {
        let key = format!("{}.{}", store.owner, store.field);
        let held_in = if store.wire {
            format!(
                "shapes `{}` records in `{}` of `{}` for the wire, as its derive declares",
                store.record, store.field, store.owner
            )
        } else {
            format!(
                "keeps `{}` records in `{}` of `{}`",
                store.record, store.field, store.owner
            )
        };
        let mut clauses = vec![store.key.as_ref().map_or_else(
            || held_in.clone(),
            |key| format!("{held_in}, each found by a `{key}`")
        )];
        if let Some(clause) = verb_clause(&kept, &key) {
            clauses.push(clause);
        }
        if let Some(clause) = command_clause(&asked, &kept) {
            clauses.push(clause);
        }
        if let Some(clause) = route_clause(&served, &key) {
            clauses.push(clause);
        }
        if store.wire {
            if let Some(clause) = answered_clause(&routes, &answered, &store.owner) {
                clauses.push(clause);
            }
            if let Some(clause) = received_clause(&routes, &received, &store.owner) {
                clauses.push(clause);
            }
            if let Some(clause) = fed_clause(&fed, &key) {
                clauses.push(clause);
            }
        }
        if let Some(clause) = telling_clause(&told, &key) {
            clauses.push(clause);
        }
        clauses.push(outlived_clause(&carried, &key));
        if let Some(clause) = &ends {
            clauses.push(clause.clone());
        }
        said.push(clauses.join("; "));
    }
    said
}

/// The printing clause of one store: the functions its records are
/// printed in.
fn telling_clause(told: &[super::telling::Telling], key: &str) -> Option<String> {
    let mut shown: Vec<String> = told
        .iter()
        .filter(|entry| entry.store == key)
        .map(|entry| format!("`{}`", entry.function))
        .collect();
    shown.sort_unstable();
    shown.dedup();
    (!shown.is_empty()).then(|| format!("what it holds is printed in {}", shown.join(", ")))
}

/// The outliving clause of one store: which methods carry it across the
/// end of the run, or the statement that nothing does.
fn outlived_clause(carried: &[super::outliving::Outliving], key: &str) -> String {
    let mut ways: Vec<String> = carried
        .iter()
        .filter(|entry| entry.store == key)
        .map(|entry| format!("`{}` {}", entry.function, entry.direction))
        .collect();
    ways.sort_unstable();
    ways.dedup();
    if ways.is_empty() {
        "nothing carries them past the end of the run".to_owned()
    } else {
        format!("{}, so they outlive the run", ways.join(", "))
    }
}

/// The ending clause: the statuses the run can leave with. Derived
/// once — the statuses belong to the run, not to any one store — and
/// repeated per sentence.
fn ending_clause(workspace: &Workspace<'_>) -> Option<String> {
    let mut ends: Vec<String> = endings(workspace)
        .iter()
        .map(|way| {
            format!(
                "{} with {}",
                if way.certain {
                    "always ends"
                } else {
                    "can end"
                },
                way.status
            )
        })
        .collect();
    ends.sort_unstable();
    ends.dedup();
    (!ends.is_empty()).then(|| format!("the run {}", ends.join(", ")))
}

/// The verb clause of one store: what the program's own methods do to
/// it.
fn verb_clause(kept: &[super::verb::Keeping], key: &str) -> Option<String> {
    let mut verbs: Vec<String> = kept
        .iter()
        .filter(|entry| entry.store == key)
        .map(|entry| format!("`{}` {}", entry.function, entry.verb))
        .collect();
    verbs.sort_unstable();
    verbs.dedup();
    (!verbs.is_empty()).then(|| format!("of its own methods, {}", verbs.join(", ")))
}

/// The command clause: the written names a run asks with, each with the
/// origin of the key it hands over where that origin was derived.
fn command_clause(
    asked: &[super::asking::Command],
    kept: &[super::verb::Keeping]
) -> Option<String> {
    let mut names: Vec<String> = asked
        .iter()
        .filter(|command| kept.iter().any(|entry| entry.function == command.reaches))
        .map(|command| {
            command.carries.as_ref().map_or_else(
                || format!("{} {}", command.written, command.verb),
                |carried| format!("{} {} named by {carried}", command.written, command.verb)
            )
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    (!names.is_empty()).then(|| format!("a run asks by name: {}", names.join(", ")))
}

/// The route clause of one store: every endpoint that reaches it,
/// stated as what a caller writes and what they get, with a route that
/// may not be mounted saying so.
fn route_clause(served: &[super::served::Serving], key: &str) -> Option<String> {
    let mut routes: Vec<String> = served
        .iter()
        .filter(|serving| serving.store == key)
        .map(|serving| {
            let mut asked = format!("`{} {}` {}", serving.method, serving.path, serving.verb);
            if let Some(name) = &serving.named_by {
                let _ = write!(asked, " named by the `{{{name}}}` the path carries");
            }
            if serving.conditional {
                asked.push_str(" (mounted in some builds or runs only)");
            }
            asked
        })
        .collect();
    routes.dedup();
    (!routes.is_empty()).then(|| format!("a request asks by route: {}", routes.join(", ")))
}

/// The answering clause of one wire shape: every route whose handler's
/// written return type is this very owner, wrappers seen through.
fn answered_clause(
    routes: &[crate::semantic::surface::Route],
    answered: &[(String, String)],
    owner: &str
) -> Option<String> {
    let mut asked: Vec<String> = routes
        .iter()
        .filter(|route| {
            answered.iter().any(|(function, with)| {
                with == owner
                    && route
                        .handler
                        .rsplit("::")
                        .next()
                        .is_some_and(|named| named == function)
            })
        })
        .map(|route| format!("`{} {}`", route.method, route.path))
        .collect();
    asked.sort_unstable();
    asked.dedup();
    (!asked.is_empty()).then(|| format!("a request is answered with it: {}", asked.join(", ")))
}

/// The receiving clause of one wire shape: every route whose handler's
/// written parameters carry this very owner, wrappers seen through.
fn received_clause(
    routes: &[crate::semantic::surface::Route],
    received: &[(String, String)],
    owner: &str
) -> Option<String> {
    let mut arrives: Vec<String> = routes
        .iter()
        .filter(|route| {
            received.iter().any(|(function, with)| {
                with == owner
                    && route
                        .handler
                        .rsplit("::")
                        .next()
                        .is_some_and(|named| named == function)
            })
        })
        .map(|route| format!("`{} {}`", route.method, route.path))
        .collect();
    arrives.sort_unstable();
    arrives.dedup();
    (!arrives.is_empty()).then(|| format!("a request arrives as it: {}", arrives.join(", ")))
}

/// The filling clause of one wire shape: the kept stores its collection
/// is derived from, each with the function whose body performs it.
fn fed_clause(fed: &[super::fed::Feeding], key: &str) -> Option<String> {
    let mut fillings: Vec<String> = fed
        .iter()
        .filter(|feeding| feeding.wire == key)
        .map(|feeding| {
            format!(
                "filled from the kept `{}` in `{}`",
                feeding.from, feeding.function
            )
        })
        .collect();
    fillings.sort_unstable();
    fillings.dedup();
    (!fillings.is_empty()).then(|| fillings.join(", "))
}
