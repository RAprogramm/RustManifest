// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How a run is told which of the things a program can do to its records
//! it should do.
//!
//! A body that chooses among written alternatives and, under one of
//! them, calls a method that touches the store, has a command surface:
//! the alternative's own pattern is what a run writes to ask for it, and
//! the method it reaches is what it gets. Both halves are already
//! recorded — the arm each call site stands under, and what each method
//! does to the store — so the reading joins them and adds nothing.
//!
//! An alternative under which no such call stands is not a command. A
//! program may choose for a hundred reasons, and only the choices that
//! reach the records are about them.

use rustmanifest_schema::Range;

use super::verb::{Verb, keeping};
use crate::semantic::{
    behavior::{Condition, FunctionFacts},
    workspace::Workspace
};

/// One written alternative that reaches the records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Function whose body writes the choice.
    pub asked_in: String,
    /// The alternative as the source writes it.
    pub written:  String,
    /// The method it reaches.
    pub reaches:  String,
    /// What that method does to the records.
    pub verb:     Verb,
    /// Where the value it hands the method came from, in words, for the
    /// one argument the site writes beside the receiver.
    pub carries:  Option<String>,
    /// Byte range of the choosing construct.
    pub site:     Range,
    /// Source the choice was read from.
    pub source:   usize
}

/// Every written alternative that reaches a method touching the records.
///
/// In the order the bodies write them, so two runs over one workspace
/// state them alike.
#[must_use]
pub fn commands(workspace: &Workspace<'_>) -> Vec<Command> {
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
    commands_with(workspace, &composed)
}

/// [`commands`] over a composition already built, so one derivation
/// serves every reading of the keeping tier instead of each site
/// rebuilding it.
#[must_use]
pub(super) fn commands_with(
    workspace: &Workspace<'_>,
    composed: &crate::semantic::summary::Composition<'_>
) -> Vec<Command> {
    let kept = keeping(workspace);
    let mut asked = Vec::new();
    for (index, fact) in workspace.facts().iter().enumerate() {
        for site in &fact.call_sites {
            let Some((choice, alternative)) = arm_of(&site.conditions) else {
                continue;
            };
            let Some(reached) = kept
                .iter()
                .find(|entry| candidate_reaches(&site.candidate, &entry.function))
            else {
                continue;
            };
            let Some(written) = written_alternative(fact, choice, alternative) else {
                continue;
            };
            if !asked_for_by_name(&written) {
                continue;
            }
            asked.push(Command {
                asked_in: fact.function.clone(),
                written,
                reaches: reached.function.clone(),
                verb: reached.verb,
                carries: handed_over(composed, index, site),
                site: choice_site(fact, choice).unwrap_or(fact.range),
                source: fact.source
            });
        }
    }
    asked
}

/// The choice and alternative a call site stands under, innermost first.
fn arm_of(conditions: &[Condition]) -> Option<(u32, u32)> {
    conditions
        .iter()
        .rev()
        .find_map(|condition| match condition {
            Condition::Arm {
                choice,
                alternative
            } => Some((*choice, *alternative)),
            _ => None
        })
}

/// Whether a written candidate names the function a keeping was read
/// from.
fn candidate_reaches(candidate: &str, function: &str) -> bool {
    candidate == function
        || candidate
            .rsplit("::")
            .next()
            .is_some_and(|named| function.rsplit("::").next() == Some(named))
}

/// The alternative as the source writes it.
fn written_alternative(fact: &FunctionFacts, choice: u32, alternative: u32) -> Option<String> {
    let recorded = fact.choices.get(usize::try_from(choice).ok()?)?;
    let written = recorded
        .alternatives
        .get(usize::try_from(alternative).ok()?)?;
    written.pattern.clone()
}

/// Where the value a command hands its method came from, in words.
///
/// A command that asks for a record by a key is asking with something
/// the run supplied, and the value tier already derived where that
/// something came from. Only the first written argument beside the
/// receiver is read: it is the one a caller writes to say *which*
/// record, and reading further would state of one argument what holds
/// of another.
fn handed_over(
    composed: &crate::semantic::summary::Composition<'_>,
    index: usize,
    site: &crate::semantic::behavior::CallSite
) -> Option<String> {
    let written = site.arguments.written()?;
    let handed = written.get(1)?;
    let term = composed.of(index, handed);
    term.derived()
        .then(|| crate::semantic::meaning::origin_text(&term))
}

/// Whether an alternative is something a run can write to ask for.
///
/// A command is named by a value the source writes down — a text, a
/// number — and matched against what the run supplied. An alternative
/// that binds a name instead is taking a value apart, which is how a
/// program reads an answer rather than how it is asked a question.
fn asked_for_by_name(written: &str) -> bool {
    let trimmed = written.trim();
    trimmed.starts_with('"') || trimmed.chars().all(|letter| letter.is_ascii_digit())
}

/// Byte range of the choosing construct a call site stands under.
fn choice_site(fact: &FunctionFacts, choice: u32) -> Option<Range> {
    Some(fact.choices.get(usize::try_from(choice).ok()?)?.site)
}
