// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How many of the recorded value positions the engine settled, and by
//! which question the rest refused.
//!
//! The value tier records a position wherever a value is used, whether or
//! not its origin was derived, so the vocabulary is far wider than the
//! coverage. Without this count a reader infers breadth from the shape of
//! the type: every refusal is a named record, and a page full of named
//! records reads like understanding. The counters state the share that is
//! actually derived, and [`origin_gaps`] names what stopped the rest.
//!
//! A position is every place a `Provenance` is recorded: the operands of
//! an operation, the scrutinee and each alternative of a choice, the value
//! of a handback, the arguments written at a call site, and the origin of
//! each hole of a written payload. A call site whose arguments are written
//! nowhere contributes no positions — an unwritten argument list is a
//! different fact from an argument whose origin failed.

use std::collections::BTreeMap;

use super::report::count;
use crate::semantic::behavior::{Arguments, FunctionFacts, Payload, Provenance, Segment};

/// How many value positions were recorded and how many were settled.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Origins {
    /// Value positions recorded across every channel.
    pub total:   u64,
    /// Positions whose origin the engine settled.
    pub derived: u64
}

/// Counts the recorded value positions of `facts` and how many were
/// settled.
pub(super) fn origins(facts: &[FunctionFacts]) -> Origins {
    let mut total = 0usize;
    let mut derived = 0usize;
    for fact in facts {
        walk(fact, &mut |origin| {
            total += 1;
            derived += usize::from(origin.derived());
        });
    }
    Origins {
        total:   count(total),
        derived: count(derived)
    }
}

/// Counts the refused value positions of `facts` by the question each one
/// failed on, keyed by the gap's own stable label.
///
/// A question absent from the map refused nowhere, which is a statement
/// about this source slice and never about the recognizer.
#[must_use]
pub fn origin_gaps(facts: &[FunctionFacts]) -> BTreeMap<&'static str, u64> {
    let mut gaps = BTreeMap::new();
    for fact in facts {
        walk(fact, &mut |origin| {
            if !origin.derived() {
                *gaps.entry(origin.label()).or_insert(0) += 1;
            }
        });
    }
    gaps
}

/// Hands every value position one function records to `seen`, in channel
/// order.
fn walk(fact: &FunctionFacts, seen: &mut impl FnMut(&Provenance)) {
    for operation in &fact.operations {
        operation.operands.iter().for_each(&mut *seen);
    }
    for choice in &fact.choices {
        seen(&choice.scrutinee);
        choice
            .alternatives
            .iter()
            .for_each(|alternative| seen(&alternative.value));
    }
    for handback in &fact.handbacks {
        seen(&handback.value);
    }
    for site in &fact.call_sites {
        if let Arguments::Written(origins) = &site.arguments {
            origins.iter().for_each(&mut *seen);
        }
    }
    for write in &fact.output_writes {
        holes(&write.payload, seen);
    }
}

/// Hands the origin of every hole of `payload` to `seen`, including the
/// holes of the alternatives a hole enumerates, which are one level deep
/// by construction.
fn holes(payload: &Payload, seen: &mut impl FnMut(&Provenance)) {
    let Payload::Template(segments) = payload else {
        return;
    };
    for segment in segments {
        let Segment::Hole(hole) = segment else {
            continue;
        };
        seen(&hole.origin);
        for alternative in &hole.alternatives {
            if let Payload::Template(inner) = alternative {
                for part in inner {
                    if let Segment::Hole(deeper) = part {
                        seen(&deeper.origin);
                    }
                }
            }
        }
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

    use super::{super::coverage, origin_gaps};
    use crate::{semantic::workspace::Workspace, source::Source};

    fn source(text: &str) -> [Source; 1] {
        [Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    #[test]
    fn a_settled_position_is_counted_as_derived() {
        let sources = source("fn main() { let one = 1; let two = one + 1; }");
        let report = coverage(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)));
        assert!(report.origins_total > 0);
        assert_eq!(report.origins_derived, report.origins_total);
        assert!(origin_gaps(Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)).facts()).is_empty());
    }

    #[test]
    fn a_rebound_name_is_counted_as_refused_and_named() {
        let sources = source("fn main() { let mut sum = 0; sum += 1; let total = sum + 1; }");
        let report = coverage(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)));
        assert!(report.origins_derived < report.origins_total);
        let gaps = origin_gaps(Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)).facts());
        assert!(gaps.values().sum::<u64>() > 0);
        assert_eq!(
            gaps.values().sum::<u64>(),
            report.origins_total - report.origins_derived
        );
    }

    #[test]
    fn a_program_recording_no_value_position_reports_none() {
        let sources = source("pub struct Empty;");
        let report = coverage(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)));
        assert_eq!(report.origins_total, 0);
        assert!(origin_gaps(Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)).facts()).is_empty());
    }
}
