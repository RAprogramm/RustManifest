// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one entry point does, as one statement or as the parts of one.
//!
//! Every clause is built from recorded facts and cites the channel it
//! rests on. A clause resting on a refusal at a load-bearing position is
//! not weakened into vagueness — the statement decomposes, and the
//! missing fact is named. That is the difference between a report that
//! hedges and one that says which question it could not answer.
//!
//! A grade below certainty is not a refusal. It degrades the wording and
//! names what weakened it, exactly as one write claim already does:
//! requiring certainty of the whole would make a compound statement
//! unreachable for any program whose later reads sit behind an earlier
//! failure — which is every program of the shape this tier was built for.

mod clause;

use self::clause::{choice_clause, compute_clause, handback_clause, intake_clause, write_clause};
use super::{EntryMeaning, completion::completion_rule};
use crate::semantic::behavior::OriginGap;

/// One part of the compound statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Clause {
    /// What the program takes in.
    Intake,
    /// What it computes.
    Computation,
    /// What it chooses between.
    Choice,
    /// What it writes.
    Write,
    /// What it hands back.
    Handback
}

impl std::fmt::Display for Clause {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Intake => "intake",
            Self::Computation => "computation",
            Self::Choice => "choice",
            Self::Write => "write",
            Self::Handback => "handback"
        })
    }
}

/// What the entry point does, as one statement or as the parts of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Narrative {
    /// Every clause is licensed, and the whole reads as one statement.
    Whole(String),
    /// At least one clause rests on a refusal. The statement decomposes
    /// into the clauses that are licensed, and names the one that is not
    /// together with the question it failed on.
    Parts {
        /// The clause that refused, and why.
        refused:  (Clause, OriginGap),
        /// The clauses that stand on their own.
        licensed: Vec<String>
    }
}

/// Builds the compound statement of one entry point.
pub(super) fn narrative_of(entry: &EntryMeaning) -> Narrative {
    let mut licensed = Vec::new();
    let mut refused = None;
    for (clause, built) in [
        (Clause::Intake, intake_clause(&entry.intake)),
        (Clause::Computation, compute_clause(&entry.computations)),
        (Clause::Choice, Ok(choice_clause(&entry.choices))),
        (Clause::Write, Ok(write_clause(&entry.claims))),
        (Clause::Handback, Ok(handback_clause(&entry.handbacks)))
    ] {
        match built {
            Ok(Some(sentence)) => licensed.push(sentence),
            Ok(None) => {}
            Err(gap) => {
                if refused.is_none() {
                    refused = Some((clause, gap));
                }
            }
        }
    }
    licensed.push(completion_rule(entry.completion).to_owned());
    match refused {
        Some(reason) => Narrative::Parts {
            refused: reason,
            licensed
        },
        None => Narrative::Whole(licensed.join("; "))
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

    use super::{Clause, Narrative};
    use crate::{
        semantic::{meaning::program_meaning, workspace::Workspace},
        source::Source
    };

    fn statement(text: &str) -> Narrative {
        let sources = [Source::new(PathBuf::from("src/main.rs"), text.to_owned())];
        program_meaning(&Workspace::new(&sources))
            .into_iter()
            .next()
            .map(|entry| entry.narrative)
            .expect("the sources declare a program")
    }

    /// The smallest program yields one whole statement, and every word of
    /// it comes from a recorded fact.
    #[test]
    fn the_smallest_program_yields_one_whole_statement() {
        let Narrative::Whole(said) = statement("fn main() { println!(\"hi\"); }") else {
            panic!("no clause rests on a refusal here");
        };
        assert!(said.contains("unconditionally writes"), "{said}");
        assert!(said.contains("ends the process with success"), "{said}");
    }

    /// A clause covering two functions makes two statements rather than
    /// one hedged to the weaker of them: certainty belongs to a site and
    /// reaches a group only through the function holding it.
    #[test]
    fn a_clause_over_two_functions_speaks_of_each_of_them() {
        let Narrative::Whole(said) = statement(
            "fn always(left: u32) -> u32 { left + 1 }\nfn guarded(flag: bool, left: u32) -> u32 { \
             if flag { return left * 2; } 0 }\nfn main() {\n    let one = always(1);\n    let \
             two = guarded(true, one);\n    println!(\"{two}\");\n}"
        ) else {
            panic!("no clause rests on a refusal here");
        };
        assert!(
            said.contains("unconditionally computes addition in `always`"),
            "{said}"
        );
        assert!(said.contains("multiplication in `guarded`"), "{said}");
    }

    /// A program taking a value from outside itself says so, and the
    /// clause names the channel and the part drawn.
    #[test]
    fn a_program_taking_a_value_in_says_which_part_of_which_channel() {
        let Narrative::Whole(said) = statement(
            "use std::env;\nfn main() {\n    let first = env::args().nth(1);\n    \
             println!(\"hi\");\n}"
        ) else {
            panic!("no clause rests on a refusal here");
        };
        assert!(
            said.contains("takes position 1 of its command-line"),
            "{said}"
        );
    }

    /// A grade below certainty degrades the wording and does not refuse:
    /// requiring certainty would make the statement unreachable for every
    /// program that guards its work.
    #[test]
    fn a_weakened_clause_is_still_licensed() {
        let Narrative::Whole(said) =
            statement("fn main(flag: bool) { if flag { println!(\"hi\"); } }")
        else {
            panic!("a weakened clause is not a refused one");
        };
        assert!(said.contains("may write"), "{said}");
    }

    /// A clause states what it states: naming the operations a program
    /// performs does not rest on where their operands came from, so an
    /// operand the reading could not settle leaves the statement whole.
    #[test]
    fn an_unsettled_operand_leaves_the_clause_that_never_names_it_whole() {
        let said = statement(
            "fn main() {\n    let mut total = 0;\n    total = 1;\n    let _ = total + 1;\n    \
             println!(\"hi\");\n}"
        );
        let Narrative::Whole(text) = said else {
            panic!("the computation clause names operations, not operands");
        };
        assert!(text.contains("computes addition"), "{text}");
        assert!(text.contains("writes"), "{text}");
    }

    /// A clause resting on a refusal it would have to speak decomposes
    /// the statement and names the question that failed, rather than
    /// hedging the whole: the intake clause says which part of a channel
    /// was read, and an adapter that does not keep positions leaves it
    /// nothing to say.
    #[test]
    fn a_refused_clause_decomposes_the_statement_and_names_the_question() {
        let said = statement(
            "fn main() {\n    let args: Vec<String> = \
             std::env::args().filter(|one| !one.is_empty()).collect();\n    println!(\"{}\", \
             args.len());\n}"
        );
        let Narrative::Parts {
            refused: (clause, _),
            licensed
        } = said
        else {
            panic!("a reading whose position the adapters lost cannot be named");
        };
        assert_eq!(clause, Clause::Intake);
        assert!(
            licensed.iter().any(|part| part.contains("writes")),
            "{licensed:?}"
        );
    }
}
