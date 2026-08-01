// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `borrowable-params` check: an owned parameter of a non-`Copy` workspace
//! type whose only observed use is reading fields should be taken by
//! reference — the function demands ownership it never exercises, forcing
//! callers to clone or give up their value.
//!
//! The reasoning is conservative on three fronts: the suggestion fires
//! only when the parameter's type is defined in the workspace (so
//! `Copy`-ness is decidable from its derives and impls); every observed
//! use is a field read AND every read field's type is provably `Copy`
//! (a field read of a non-`Copy` type may be a move out, as in
//! accessor-style tails); and consuming-by-convention methods (`into_*`)
//! are exempt entirely.

use rustmanifest_schema::TraceStep;

use super::{TypeGraph, behavior::ParamMode, insight::Insight};
use crate::source::Source;

/// Reports owned parameters that could be shared borrows, one insight per
/// function with at least one such parameter.
pub(super) fn insights(sources: &[Source], graph: &TypeGraph) -> Vec<Insight> {
    let mut found = Vec::new();
    for fact in super::behavior::behavior(sources) {
        if fact.trait_impl || consuming_by_convention(&fact.function) {
            continue;
        }
        let candidates: Vec<(&str, &str)> = fact
            .params
            .iter()
            .filter(|param| {
                param.mode == ParamMode::Owned
                    && param.uses > 0
                    && param.uses == param.field_reads
                    && is_non_copy_workspace_type(graph, &param.type_name)
                    && read_fields_are_copy(graph, &fact.reads, &param.type_name)
            })
            .map(|param| (param.name.as_str(), param.type_name.as_str()))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let listed = candidates
            .iter()
            .map(|(name, type_name)| format!("`{name}: {type_name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let detail = format!(
            "fn `{}` takes {listed} by value but only reads fields; take a shared reference \
             instead so callers keep ownership",
            fact.function
        );
        let mut trace = Vec::new();
        for (name, type_name) in &candidates {
            trace.push(TraceStep {
                fact:   format!("param `{name}: {type_name}` is taken by value"),
                reason: "signature declares no reference".to_owned()
            });
            trace.push(TraceStep {
                fact:   format!("every observed use of `{name}` is a field read"),
                reason: "no move, write, or method call was seen in the body".to_owned()
            });
            trace.push(TraceStep {
                fact:   format!("`{type_name}` is a workspace type without `Copy`"),
                reason: "derives and impls in the type graph show no Copy".to_owned()
            });
        }
        found.push(Insight::new(fact.source, fact.range, detail, trace));
    }
    found
}

/// Returns whether the function name follows a consuming convention
/// (`into_*`), where taking ownership is the point.
fn consuming_by_convention(function: &str) -> bool {
    function
        .rsplit("::")
        .next()
        .is_some_and(|name| name.starts_with("into"))
}

/// Returns whether `type_name` is defined in the workspace and provably
/// not `Copy` there.
fn is_non_copy_workspace_type(graph: &TypeGraph, type_name: &str) -> bool {
    let Some(node) = graph.type_named(type_name) else {
        return false;
    };
    !node.derives.iter().any(|name| name == "Copy")
        && !graph.traits_implemented(type_name).contains(&"Copy")
}

/// Scalar built-in types that are always `Copy`.
const COPY_BUILTINS: [&str; 17] = [
    "bool", "char", "f32", "f64", "i128", "i16", "i32", "i64", "i8", "isize", "u128", "u16",
    "u32", "u64", "u8", "usize", "str"
];

/// Returns whether every field of `owner` read in `reads` has a provably
/// `Copy` type — a field read of a non-`Copy` type may be a move out.
fn read_fields_are_copy(graph: &TypeGraph, reads: &[String], owner: &str) -> bool {
    let Some(node) = graph.type_named(owner) else {
        return false;
    };
    reads
        .iter()
        .filter_map(|read| read.strip_prefix(owner)?.strip_prefix('.'))
        .all(|field_name| {
            node.fields
                .iter()
                .find(|field| field.name == field_name)
                .is_some_and(|field| {
                    COPY_BUILTINS.contains(&field.type_name.as_str())
                        || graph
                            .type_named(&field.type_name)
                            .is_some_and(|target| target.derives.iter().any(|name| name == "Copy"))
                })
        })
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

    use super::insights;
    use crate::{semantic::TypeGraph, source::Source};

    fn found(text: &str) -> usize {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let graph = TypeGraph::from_sources(&sources);
        insights(&sources, &graph).len()
    }

    #[test]
    fn owned_read_only_param_of_non_copy_type_is_reported() {
        assert_eq!(
            found(
                "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                 report.total }"
            ),
            1
        );
    }

    #[test]
    fn borrowed_param_passes() {
        assert_eq!(
            found(
                "struct Report { total: u64 }\nfn summarize(report: &Report) -> u64 { \
                 report.total }"
            ),
            0
        );
    }

    #[test]
    fn copy_types_are_exempt() {
        assert_eq!(
            found(
                "#[derive(Clone, Copy)]\nstruct Coord { x: u8 }\nfn dx(coord: Coord) -> u8 { \
                 coord.x }"
            ),
            0
        );
    }

    #[test]
    fn moved_param_is_not_flagged() {
        assert_eq!(
            found("struct Report { total: u64 }\nfn pass(report: Report) -> Report { report }"),
            0
        );
    }

    #[test]
    fn unknown_external_types_are_not_judged() {
        assert_eq!(found("fn show(s: String) -> usize { s.len() }"), 0);
    }

    #[test]
    fn tuple_field_access_is_not_a_proven_read() {
        assert_eq!(
            found("struct Paths(Vec<u8>);\nimpl Paths { fn list(self) -> Vec<u8> { self.0 } }"),
            0
        );
    }

    #[test]
    fn trait_impl_methods_are_exempt() {
        assert_eq!(
            found(
                "struct Wrapper { inner: u64 }\nimpl From<Wrapper> for u64 { fn from(wrapper: \
                 Wrapper) -> u64 { wrapper.inner } }"
            ),
            0
        );
    }

    #[test]
    fn consuming_convention_methods_are_exempt() {
        assert_eq!(
            found(
                "struct Field { value: u64 }\nimpl Field { fn into_value(self) -> u64 { \
                 self.value } }"
            ),
            0
        );
    }

    #[test]
    fn reading_a_non_copy_field_may_be_a_move_and_is_not_flagged() {
        assert_eq!(
            found(
                "struct User { name: String }\nfn take_name(user: User) -> String { user.name }"
            ),
            0
        );
    }

    #[test]
    fn trace_explains_all_three_conditions() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { report.total }"
                .to_owned()
        )];
        let graph = TypeGraph::from_sources(&sources);
        let insights = insights(&sources, &graph);
        assert_eq!(insights[0].trace.len(), 3);
        assert!(insights[0].trace[2].fact.contains("without `Copy`"));
    }
}
