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

use super::{
    asking::commands, ending::endings, outliving::outliving, store::stores, telling::telling,
    verb::keeping
};
use crate::semantic::workspace::Workspace;

/// What a program does with the records it holds, in one sentence per
/// store.
///
/// Empty where the workspace holds no records: silence is "no store
/// proven", never "this program is about nothing".
#[must_use]
pub fn purpose(workspace: &Workspace<'_>) -> Vec<String> {
    let held = stores(workspace);
    let kept = keeping(workspace);
    let carried = outliving(workspace);
    let asked = commands(workspace);
    let told = telling(workspace);
    let mut said = Vec::new();
    for store in &held {
        let key = format!("{}.{}", store.owner, store.field);
        let held_in = format!(
            "keeps `{}` records in `{}` of `{}`",
            store.record, store.field, store.owner
        );
        let mut clauses = vec![store.key.as_ref().map_or_else(
            || held_in.clone(),
            |key| format!("{held_in}, each found by a `{key}`")
        )];
        let mut verbs: Vec<String> = kept
            .iter()
            .filter(|entry| entry.store == key)
            .map(|entry| format!("`{}` {}", entry.function, entry.verb))
            .collect();
        verbs.sort_unstable();
        verbs.dedup();
        if !verbs.is_empty() {
            clauses.push(format!("of its own methods, {}", verbs.join(", ")));
        }
        let mut names: Vec<String> = asked
            .iter()
            .filter(|command| kept.iter().any(|entry| entry.function == command.reaches))
            .map(|command| match &command.carries {
                Some(carried) => format!("{} {} named by {carried}", command.written, command.verb),
                None => format!("{} {}", command.written, command.verb)
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        if !names.is_empty() {
            clauses.push(format!("a run asks by name: {}", names.join(", ")));
        }
        let mut ways: Vec<String> = carried
            .iter()
            .filter(|entry| entry.store == key)
            .map(|entry| format!("`{}` {}", entry.function, entry.direction))
            .collect();
        ways.sort_unstable();
        ways.dedup();
        let mut shown: Vec<String> = told
            .iter()
            .filter(|entry| entry.store == key)
            .map(|entry| format!("`{}`", entry.function))
            .collect();
        shown.sort_unstable();
        shown.dedup();
        if !shown.is_empty() {
            clauses.push(format!("what it holds is printed in {}", shown.join(", ")));
        }
        if ways.is_empty() {
            clauses.push("nothing carries them past the end of the run".to_owned());
        } else {
            clauses.push(format!("{}, so they outlive the run", ways.join(", ")));
        }
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
        if !ends.is_empty() {
            clauses.push(format!("the run {}", ends.join(", ")));
        }
        said.push(clauses.join("; "));
    }
    said
}
