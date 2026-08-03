// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program does for an observer outside the process.
//!
//! One claim per recorded write site reachable from an entry point: the
//! stream it goes to, the text it puts there, and how certain both are.
//! Certainty has two independent parts that are never merged into a
//! score — whether the owning function is reached from the entry, and
//! whether the site runs on every path through its own function. The
//! rendered sentence degrades field by field: a weakened certainty names
//! the construct that weakened it, a partially proven payload prints its
//! proven text with the holes left as holes, and an unidentifiable
//! stream says so instead of guessing.
//!
//! Two properties of the standard library, not of the analyzed program,
//! are attached to every claim about a standard stream: print output can
//! be diverted by the library's capture hook, and a failed write panics
//! rather than writing. A claim is therefore about an attempted write.

use std::{collections::BTreeSet, fmt::Write as _};

use super::{
    behavior::{
        FunctionFacts, Grade, Guard, Multiplicity, Newline, OUTPUT_SINK_SET, OpaqueReason,
        OutputWrite, Payload, Segment, Stream, behavior
    },
    calls::{CallGraph, call_graph, resolution_context},
    entry::{EntryKind, EntryPoint, entry_points}
};
use crate::source::Source;

/// One statement about what the program writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputClaim {
    /// Function holding the write site.
    pub function:  String,
    /// File the site lives in.
    pub file:      String,
    /// The recorded write.
    pub write:     OutputWrite,
    /// How certainly the owning function is reached from the entry.
    pub reach:     Grade,
    /// How certainly the site runs, both parts composed.
    pub execution: Grade,
    /// Version of the recognizer list that produced this claim.
    pub sink_set:  u32
}

/// Everything one entry point is observed to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeaning {
    /// Path of the binary root declaring the entry.
    pub file:   String,
    /// How the entry is declared.
    pub kind:   EntryKind,
    /// Claims in the order their sites were recorded.
    pub claims: Vec<OutputClaim>
}

/// Derives what every entry point of `sources` does.
#[must_use]
pub fn program_meaning(sources: &[Source]) -> Vec<EntryMeaning> {
    let facts = behavior(sources);
    let graph = call_graph(&facts, &resolution_context(sources));
    entry_points(sources, &facts)
        .into_iter()
        .map(|entry| meaning_of(&entry, &facts, &graph, sources))
        .collect()
}

/// Builds the claims of one entry point.
fn meaning_of(
    entry: &EntryPoint,
    facts: &[FunctionFacts],
    graph: &CallGraph,
    sources: &[Source]
) -> EntryMeaning {
    let reach = reachability(facts, graph, entry.fact);
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        if reach[index] == Grade::Unreached {
            continue;
        }
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
                sink_set:  OUTPUT_SINK_SET
            });
        }
    }
    EntryMeaning {
        file: entry.file.clone(),
        kind: entry.kind,
        claims
    }
}

/// Grades how certainly every function is reached from `entry`.
///
/// The entry itself is certain. A callee is as certain as its caller
/// when the caller reaches it through an edge that is neither a
/// possibility over several implementors nor made from a call standing
/// under a branch — otherwise the callee is at most possible. Unreached
/// functions keep their initial grade, so the caller can skip them.
fn reachability(facts: &[FunctionFacts], graph: &CallGraph, entry: usize) -> Vec<Grade> {
    let mut grades = vec![Grade::Unreached; facts.len()];
    if entry >= facts.len() {
        return grades;
    }
    grades[entry] = Grade::Certain;
    let mut changed = true;
    while changed {
        changed = false;
        for caller in 0..facts.len() {
            if grades[caller] == Grade::Unreached {
                continue;
            }
            for &callee in graph.callees(caller) {
                let edge = if graph.possibilistic(caller, callee) {
                    Grade::Possible
                } else {
                    certainty_of_call(&facts[caller], &facts[callee].function)
                };
                let candidate = grades[caller].min(edge);
                if candidate > grades[callee] {
                    grades[callee] = candidate;
                    changed = true;
                }
            }
        }
    }
    grades
}

/// How certainly a caller reaches one callee, from the call sites it
/// recorded. A caller with no unconditional call to that name reaches it
/// at most possibly.
fn certainty_of_call(from: &FunctionFacts, target: &str) -> Grade {
    let method = target.rsplit("::").next().unwrap_or(target);
    let unconditional = from.call_sites.iter().any(|site| {
        (site.candidate == target || site.candidate.ends_with(&format!("::{method}")))
            && site.guard.local_grade() == Grade::Certain
    });
    if unconditional {
        Grade::Certain
    } else {
        Grade::Possible
    }
}

impl EntryMeaning {
    /// Renders every claim as one sentence, strongest form first, each
    /// degraded exactly as far as its weakest field allows.
    #[must_use]
    pub fn sentences(&self) -> Vec<String> {
        if self.claims.is_empty() {
            return vec![format!(
                "no recognized write to a standard stream (recognizer set v{OUTPUT_SINK_SET}); \
                 this states the recognizer found none, not that the program writes nothing"
            )];
        }
        self.claims.iter().map(OutputClaim::sentence).collect()
    }
}

impl OutputClaim {
    /// The sentence this claim licenses.
    #[must_use]
    pub fn sentence(&self) -> String {
        let verb = match self.execution {
            Grade::Certain => "unconditionally writes",
            Grade::Possible => "may write",
            Grade::Dead => "contains a write that can never run:",
            Grade::Unreached => "is never reached, so it never writes"
        };
        let destination = match self.write.stream {
            Stream::Stdout => "its standard output stream",
            Stream::Stderr => "its standard error stream",
            Stream::CallerSupplied => "a stream the caller supplies",
            Stream::Unknown => "a stream this analysis cannot identify"
        };
        let mut sentence = format!(
            "the process {verb} {} to {destination}",
            describe(&self.write.payload, self.write.newline)
        );
        if self.write.multiplicity == Multiplicity::Unbounded {
            sentence.push_str(", once per iteration of the loop around it");
        }
        if self.execution == Grade::Possible {
            let _ = write!(sentence, ", weakened by {}", weakening(self));
        }
        let _ = write!(
            sentence,
            " [{} in `{}`, attempted: the library can divert or fail the write]",
            self.file, self.function
        );
        sentence
    }
}

/// Names what stands between a claim and certainty.
fn weakening(claim: &OutputClaim) -> String {
    if claim.reach != Grade::Certain {
        return "a call chain that does not certainly reach it".to_owned();
    }
    let guards: BTreeSet<&'static str> = claim
        .write
        .guard
        .guards
        .iter()
        .map(|guard| match guard {
            Guard::If | Guard::Else => "a branch",
            Guard::MatchArm {
                ..
            } => "a match arm",
            Guard::For | Guard::While => "a loop that may not run",
            Guard::Closure => "a closure that runs only where it is called",
            Guard::NestedItem => "a nested item that runs only when called",
            Guard::CfgGated => "a configuration gate this analysis does not resolve"
        })
        .collect();
    if guards.is_empty() {
        return "an exit that may leave the function first".to_owned();
    }
    guards.into_iter().collect::<Vec<_>>().join(" and ")
}

/// Renders a payload without ever claiming an unproven value.
fn describe(payload: &Payload, newline: Newline) -> String {
    let terminator = match newline {
        Newline::Appended => " followed by a line terminator",
        Newline::None => ""
    };
    match payload {
        Payload::Empty => "nothing".to_owned(),
        Payload::Literal(text) => format!("the string `{text}`{terminator}"),
        Payload::Template(segments) => {
            let shape: String = segments
                .iter()
                .map(|segment| match segment {
                    Segment::Text(text) => text.clone(),
                    Segment::Hole(hole) => format!("<hole:{}>", hole_name(hole))
                })
                .collect();
            format!(
                "text of the form `{shape}`{terminator}, where each hole is a runtime value this \
                 analysis does not determine"
            )
        }
        Payload::Opaque(reason) => format!(
            "a value this analysis cannot read ({})",
            match reason {
                OpaqueReason::RuntimeValue => "the written value is computed at runtime",
                OpaqueReason::NonLiteralFormat => "the format string is not a literal",
                OpaqueReason::Unparsed => "the arguments did not parse",
                OpaqueReason::DebugMacro => "the debugging macro embeds its own source position"
            }
        )
    }
}

/// Names one hole by whatever the source gives it.
fn hole_name(hole: &super::behavior::Hole) -> String {
    match &hole.source {
        super::behavior::HoleSource::Named(name) => name.clone(),
        super::behavior::HoleSource::Positional(index)
        | super::behavior::HoleSource::Indexed(index) => hole
            .expr
            .clone()
            .unwrap_or_else(|| format!("argument {index}"))
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

    use super::program_meaning;
    use crate::source::Source;

    fn sentences(body: &str) -> Vec<String> {
        let sources = [Source::new(PathBuf::from("src/main.rs"), body.to_owned())];
        program_meaning(&sources)
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
}
