// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program does for an observer outside the process.
//!
//! One claim per recorded write site, reachable from an entry point or
//! not: the stream it goes to, the text it puts there, and how certain
//! both are. Certainty has two independent parts that are never merged
//! into a score — whether the owning function is reached from the entry,
//! and whether the site runs on every path through its own function. A
//! site the call graph never reaches keeps its claim and states that,
//! because dropping it would turn a fact about the call graph into an
//! empty report about the recognizer. The rendered sentence degrades
//! field by field: a weakened certainty names every construct that
//! weakened it, a partially proven payload prints its proven text with
//! the holes left as holes, and an unidentifiable stream says so instead
//! of guessing.
//!
//! Two properties of the standard library, not of the analyzed program,
//! are attached to every claim about a standard stream: print output can
//! be diverted by the library's capture hook, and a failed write panics
//! rather than writing. A claim is therefore about an attempted write.

mod claim;
mod completion;
mod intake;
mod narrative;
mod origin_text;
mod output_claim;
mod payload;
mod reach;
mod scope;
mod sentence;
mod weakening;

use std::collections::BTreeSet;

pub(in crate::semantic) use self::scope::qualifiers_by_fact;
pub use self::{
    claim::{ChoiceClaim, ComputeClaim, HandbackClaim},
    completion::{ExitClaim, completion_rule},
    intake::IntakeClaim,
    narrative::{Clause, Narrative},
    origin_text::origin_text,
    output_claim::OutputClaim
};
use self::{output_claim::hole_sources, reach::reachability};
use super::{
    behavior::{FunctionFacts, Grade, OUTPUT_SINK_SET},
    calls::CallGraph,
    entry::{Completion, EntryKind, EntryPoint, entry_points}
};
use crate::{semantic::workspace::Workspace, source::Source};

/// Everything one entry point is observed to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeaning {
    /// Path of the binary root declaring the entry.
    pub file:         String,
    /// How the entry is declared.
    pub kind:         EntryKind,
    /// Claims in the order their sites were recorded, one per recorded
    /// write, including the writes no path from this entry reaches —
    /// those carry [`Grade::Unreached`] rather than being dropped.
    pub claims:       Vec<OutputClaim>,
    /// One claim per recorded read from outside the process, on the same
    /// terms as the write claims.
    pub intake:       Vec<IntakeClaim>,
    /// One claim per recorded operation, on the same terms.
    pub computations: Vec<ComputeClaim>,
    /// One claim per recorded choice, on the same terms.
    pub choices:      Vec<ChoiceClaim>,
    /// One claim per recorded return position, on the same terms.
    pub handbacks:    Vec<HandbackClaim>,
    /// What the entry point does, as one statement or as the parts of
    /// one.
    pub narrative:    Narrative,
    /// What the entry's own signature decides about the status the
    /// process leaves behind.
    pub completion:   Completion,
    /// One claim per recorded site producing a status, on the same terms
    /// as the write claims.
    pub exits:        Vec<ExitClaim>
}

/// Derives what every entry point of `workspace` does.
#[must_use]
pub fn program_meaning(workspace: &Workspace<'_>) -> Vec<EntryMeaning> {
    let sources = workspace.sources();
    let facts = workspace.facts();
    let graph = workspace.calls();
    let qualifiers = qualifiers_by_fact(sources, facts);
    let composed = crate::semantic::summary::composition_with(
        sources,
        workspace.resolution(),
        facts,
        graph,
        &qualifiers
    );
    entry_points(sources, facts)
        .into_iter()
        .map(|entry| meaning_of(&entry, facts, graph, &qualifiers, sources, &composed))
        .collect()
}

/// Builds the claims of one entry point.
///
/// Every recorded write becomes a claim. A write the call graph does not
/// reach from this entry is stated as unreached, not dropped: dropping
/// it would leave an empty list that reads as "the recognizer found
/// nothing", which is a different fact and, here, a false one.
fn meaning_of(
    entry: &EntryPoint,
    facts: &[FunctionFacts],
    graph: &CallGraph,
    qualifiers: &[BTreeSet<String>],
    sources: &[Source],
    composed: &crate::semantic::summary::Composition<'_>
) -> EntryMeaning {
    let reach = reachability(facts, graph, qualifiers, entry.fact);
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = sources.get(fact.source).map_or_else(String::new, |source| {
            source.path().to_string_lossy().replace('\\', "/")
        });
        for write in &fact.output_writes {
            claims.push(OutputClaim {
                function:  fact.function.clone(),
                file:      file.clone(),
                execution: reach[index].min(write.guard.local_grade()),
                reach:     reach[index],
                write:     write.clone(),
                sources:   hole_sources(composed, index, write),
                sink_set:  OUTPUT_SINK_SET
            });
        }
    }
    let mut meaning = EntryMeaning {
        file: entry.file.clone(),
        kind: entry.kind,
        claims,
        completion: entry.completion,
        intake: intake::intake_claims(facts, sources, &reach),
        computations: claim::compute_claims(facts, sources, &reach),
        choices: claim::choice_claims(facts, sources, &reach),
        handbacks: claim::handback_claims(facts, sources, &reach),
        narrative: Narrative::Whole(String::new()),
        exits: exit_claims(facts, sources, &reach)
    };
    meaning.narrative = narrative::narrative_of(&meaning);
    meaning
}

/// Builds the status claims of one entry point, one per recorded site.
///
/// A site no path from this entry reaches is stated as unreached rather
/// than dropped, on the same grounds as an unreached write: the absence
/// of a path is itself the finding.
fn exit_claims(facts: &[FunctionFacts], sources: &[Source], reach: &[Grade]) -> Vec<ExitClaim> {
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = sources.get(fact.source).map_or_else(String::new, |source| {
            source.path().to_string_lossy().replace('\\', "/")
        });
        for exit in &fact.exits {
            claims.push(ExitClaim {
                function:  fact.function.clone(),
                file:      file.clone(),
                execution: reach[index].min(exit.guard.local_grade()),
                reach:     reach[index],
                exit:      exit.clone()
            });
        }
    }
    claims
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

    use super::program_meaning;
    use crate::{
        semantic::{
            module_graph::declared_tree, settling::settle_identity, workspace::Workspace
        },
        source::Source
    };

    /// The module tree and the call identity a compiler settles for
    /// `sources`.
    fn settled(sources: &[Source]) -> rustmanifest_schema::Resolution {
        let mut resolution = declared_tree(sources);
        settle_identity(
            &mut resolution,
            sources,
            &crate::semantic::behavior::behavior(sources)
        );
        resolution
    }

    fn sentences(body: &str) -> Vec<String> {
        sentences_of(&[("src/main.rs", body)])
    }

    /// Every sentence of the first entry point of a whole file set, so a
    /// claim about cross-file qualification is stated on the layout it
    /// depends on.
    fn sentences_of(files: &[(&str, &str)]) -> Vec<String> {
        let sources: Vec<Source> = files
            .iter()
            .map(|(path, body)| Source::new(PathBuf::from(path), (*body).to_owned()))
            .collect();
        program_meaning(&Workspace::resolved(&sources, &settled(&sources)))
            .first()
            .map(super::EntryMeaning::sentences)
            .unwrap_or_default()
    }

    #[test]
    fn the_smallest_program_yields_the_strongest_sentence() {
        let said = sentences("fn main() {\n    println!(\"Hello, world!\");\n}");
        assert_eq!(said.len(), 1);
        assert!(
            said[0].starts_with(
                "the process unconditionally writes the string `Hello, world!` followed by a \
                 line terminator to its standard output stream"
            ),
            "{}",
            said[0]
        );
    }

    #[test]
    fn a_branch_degrades_the_sentence_and_names_what_weakened_it() {
        let said = sentences("fn main() {\n    if flag { println!(\"Hello\"); }\n}");
        assert!(said[0].contains("may write"), "{}", said[0]);
        assert!(said[0].contains("weakened by a branch"), "{}", said[0]);
    }

    #[test]
    fn a_runtime_value_is_never_rendered_as_text() {
        let said = sentences("fn main() {\n    println!(\"hi {name}\");\n}");
        assert!(
            said[0].contains("text of the form `hi <hole:name>`"),
            "{}",
            said[0]
        );
        assert!(
            said[0].contains("this analysis does not determine"),
            "{}",
            said[0]
        );
    }

    #[test]
    fn a_write_behind_a_call_keeps_its_certainty_across_the_boundary() {
        let said =
            sentences("fn greet() { println!(\"Hello, world!\"); }\nfn main() {\n    greet();\n}");
        assert!(said[0].contains("unconditionally writes"), "{}", said[0]);
        assert!(said[0].contains("in `greet`"), "{}", said[0]);
    }

    #[test]
    fn a_program_without_a_recognized_write_says_so_without_claiming_silence() {
        let said = sentences("fn main() {\n    let value = 1 + 1;\n}");
        assert_eq!(said.len(), 1);
        assert!(said[0].starts_with("no recognized write"), "{}", said[0]);
        assert!(said[0].contains("not that the program writes nothing"));
    }

    #[test]
    fn a_loop_reports_repetition_rather_than_a_single_write() {
        let said = sentences("fn main() {\n    for _ in 0..3 { println!(\"tick\"); }\n}");
        assert!(said[0].contains("once per iteration"), "{}", said[0]);
    }

    /// The one sentence mentioning `text`, so a test names the claim it
    /// is about instead of an index into a list that may grow.
    fn about(said: &[String], text: &str) -> String {
        let mut matching = said.iter().filter(|sentence| sentence.contains(text));
        let found = matching
            .next()
            .unwrap_or_else(|| panic!("no sentence about `{text}` in {said:#?}"));
        assert!(
            matching.next().is_none(),
            "several sentences about `{text}` in {said:#?}"
        );
        found.clone()
    }

    #[test]
    fn a_call_on_one_type_never_certifies_the_same_method_name_on_another() {
        let said = sentences(
            "struct Ready;\nstruct Lazy;\nimpl Ready {\n    fn go() { println!(\"ready\"); \
             }\n}\nimpl Lazy {\n    fn go() { println!(\"lazy\"); }\n}\nfn main() {\n    \
             Ready::go();\n    if flag { Lazy::go(); }\n}"
        );
        let ready = about(&said, "ready");
        let lazy = about(&said, "lazy");
        assert!(ready.contains("unconditionally writes"), "{ready}");
        assert!(
            lazy.contains("may write"),
            "the only call to `Lazy::go` stands under a branch: {lazy}"
        );
    }

    #[test]
    fn an_unsettled_receiver_certifies_nothing_about_the_method_it_might_reach() {
        let said = sentences(
            "struct Lazy;\nimpl Lazy {\n    fn go(&self) { println!(\"lazy\"); }\n}\nfn main() \
             {\n    let value = elsewhere();\n    value.go();\n}"
        );
        let lazy = about(&said, "lazy");
        assert!(
            lazy.contains("no path from the entry point reaches"),
            "no resolver said which body the call reaches, so the entry reaches none: {lazy}"
        );
    }

    #[test]
    fn a_function_called_only_from_a_dead_site_writes_nothing() {
        let said = sentences(
            "fn shout() { println!(\"never\"); }\nfn main() {\n    return;\n    shout();\n}"
        );
        let never = about(&said, "never");
        assert!(never.contains("can never run"), "{never}");
    }

    #[test]
    fn a_claim_weakened_twice_names_both_weakeners() {
        let said = sentences(
            "fn shout() {\n    if inner { println!(\"hi\"); }\n}\nfn main() {\n    if outer { \
             shout(); }\n}"
        );
        let claim = about(&said, "hi");
        assert!(
            claim.contains("a call chain that does not certainly reach it"),
            "{claim}"
        );
        assert!(claim.contains("a branch"), "{claim}");
    }

    #[test]
    fn a_qualifier_naming_another_module_never_certifies_the_call() {
        let said = sentences(
            "mod alpha {\n    pub fn go() { println!(\"alpha\"); }\n}\nmod beta {\n    pub fn \
             stub() { let value = 1; }\n}\nfn main() {\n    beta::go();\n}"
        );
        let alpha = about(&said, "alpha");
        assert!(
            !alpha.contains("unconditionally writes"),
            "`beta::go` is not `alpha::go`; the qualifier does not own the callee: {alpha}"
        );
    }

    #[test]
    fn the_module_that_declares_the_function_still_certifies_the_call() {
        let said = sentences(
            "mod alpha {\n    pub fn go() { println!(\"alpha\"); }\n}\nfn main() {\n    \
             alpha::go();\n}"
        );
        let alpha = about(&said, "alpha");
        assert!(
            alpha.contains("unconditionally writes"),
            "the qualifier names the very module declaring the callee: {alpha}"
        );
    }

    #[test]
    fn a_file_module_certifies_its_own_function_and_no_other_files() {
        let said = sentences_of(&[
            (
                "src/main.rs",
                "mod alpha;\nmod beta;\nfn main() {\n    alpha::go();\n}"
            ),
            ("src/alpha.rs", "pub fn go() { println!(\"alpha\"); }"),
            ("src/beta.rs", "pub fn stub() { let value = 1; }")
        ]);
        let alpha = about(&said, "alpha");
        assert!(
            alpha.contains("unconditionally writes"),
            "the file is the module the qualifier names: {alpha}"
        );
        let other = sentences_of(&[
            (
                "src/main.rs",
                "mod alpha;\nmod beta;\nfn main() {\n    beta::go();\n}"
            ),
            ("src/alpha.rs", "pub fn go() { println!(\"alpha\"); }"),
            ("src/beta.rs", "pub fn stub() { let value = 1; }")
        ]);
        let claimed = about(&other, "alpha");
        assert!(
            !claimed.contains("unconditionally writes"),
            "`beta` does not declare `go`: {claimed}"
        );
    }

    #[test]
    fn a_dead_claim_names_what_killed_it() {
        let said = sentences(
            "fn shout() { println!(\"never\"); }\nfn main() {\n    return;\n    shout();\n}"
        );
        let never = about(&said, "never");
        assert!(
            never.contains("a call chain whose every call to it can never run"),
            "a claim that can never run states its cause like every other: {never}"
        );
    }

    #[test]
    fn an_unreached_claim_names_the_site_without_restating_the_chain() {
        let said =
            sentences("fn orphan() {\n    if flag { println!(\"orphan\"); }\n}\nfn main() {}");
        let orphan = about(&said, "orphan");
        assert!(orphan.contains("weakened by a branch"), "{orphan}");
        assert!(
            !orphan.contains("a call chain that never reaches it"),
            "the opening already states the chain; saying it twice states nothing new: {orphan}"
        );
    }

    #[test]
    fn a_write_the_call_graph_never_reaches_is_stated_rather_than_dropped() {
        let said = sentences("fn orphan() { println!(\"orphan\"); }\nfn main() {}");
        assert_eq!(said.len(), 1, "{said:#?}");
        assert!(
            said[0].contains("no path from the entry point reaches"),
            "{}",
            said[0]
        );
        assert!(
            !said[0].starts_with("no recognized write"),
            "a recognized write was found; the recognizer set is not what emptied the report"
        );
    }
}
