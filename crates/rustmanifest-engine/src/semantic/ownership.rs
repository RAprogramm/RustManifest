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

use std::collections::BTreeMap;

use rustmanifest_schema::TraceStep;

use super::{
    TypeGraph, behavior::ParamMode, insight::Insight, layout::shapes_with, workspace::Workspace
};

/// Reports owned parameters that could be shared borrows, one insight per
/// function with at least one such parameter.
pub(super) fn insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let graph = workspace.types();
    let weights: BTreeMap<String, (Option<u64>, u32)> =
        shapes_with(sources, workspace.resolution())
            .into_iter()
            .map(|shape| (shape.name, (shape.payload, shape.indirections)))
            .collect();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        if fact.flags.trait_impl() || consuming_by_convention(&fact.function) {
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
                    && read_fields_are_copy(
                        graph,
                        workspace.resolution(),
                        &fact.reads,
                        &param.type_name
                    )
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
        let priced: Vec<(u64, u32)> = candidates
            .iter()
            .filter_map(|(_, type_name)| {
                weights
                    .get(*type_name)
                    .map(|(payload, indirections)| (payload.unwrap_or(0), *indirections))
            })
            .collect();
        let copied_bytes: u64 = priced.iter().map(|(payload, _)| payload).sum();
        let dragged_heaps: u32 = priced.iter().map(|(_, indirections)| indirections).sum();
        let price = if copied_bytes > 0 || dragged_heaps > 0 {
            format!(
                "; the by-value pass provably moves at least {copied_bytes} bytes and {dragged_heaps} \
                 heap header(s) per call"
            )
        } else {
            String::new()
        };
        let detail = format!(
            "fn `{}` takes {listed} by value but only reads fields; take a shared reference \
             instead so callers keep ownership{price}",
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
            if let Some((Some(payload), indirections)) = weights.get(*type_name) {
                trace.push(TraceStep {
                    fact:   format!(
                        "`{type_name}` weighs {payload} guaranteed field byte(s) and \
                         {indirections} heap header(s)"
                    ),
                    reason: "memory-shape facts derived from the definition".to_owned()
                });
            }
        }
        let fixes = if let [(param_name, _)] = candidates.as_slice() {
            super::borrow_fix::borrow_fixes(
                sources,
                &fact.function,
                param_name,
                fact.flags.public()
            )
        } else {
            Vec::new()
        };
        found.push(Insight::new(fact.source, fact.range, detail, trace).with_fixes(fixes));
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
    graph.type_named(type_name).is_some() && !graph.ability_given(type_name, "Copy")
}

/// Returns whether every field of `owner` read in `reads` has a provably
/// `Copy` type — a field read of a non-`Copy` type may be a move out.
///
/// Whether a type is copied is the compiler's answer, and a resolver that
/// ran carries it. Where none ran, only a derive written in this workspace
/// proves it: a list of standard type names would answer for the names
/// someone thought to write down and stay silent about every other, which
/// is the same silence with a longer maintenance bill.
fn read_fields_are_copy(
    graph: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>,
    reads: &[String],
    owner: &str
) -> bool {
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
                    granted_copy(settled, &field.type_name)
                        || graph
                            .type_named(&field.type_name)
                            .is_some_and(|target| graph.ability_given(&target.name, "Copy"))
                })
        })
}

/// Whether values of the type named `named` are copied rather than moved.
///
/// The language's own primitives answer for themselves. Every other type
/// is the compiler's answer, carried by a resolver that ran; where none
/// ran, the question is unanswered rather than guessed from a list of
/// library names somebody remembered to write down.
fn granted_copy(settled: Option<&rustmanifest_schema::Resolution>, named: &str) -> bool {
    if super::std_knowledge::is_primitive_copy(named) {
        return true;
    }
    settled.is_some_and(|resolution| {
        resolution.types_granted.iter().any(|row| {
            row.copy && super::behavior::outermost_name(&row.named).as_deref() == Some(named)
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
    use crate::{
        semantic::{TypeGraph, settling::Settling, workspace::Workspace},
        source::Source
    };

    fn found(text: &str) -> usize {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let _graph = TypeGraph::from_sources(&sources);
        insights(&Workspace::new(&sources)).len()
    }

    /// The sizes a compiler states for the primitives and headers these
    /// fixtures write.
    fn weighed() -> rustmanifest_schema::Resolution {
        let mut settled = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            layouts: [("String", 24u64), ("u64", 8), ("Vec", 24), ("i64", 8)]
                .into_iter()
                .map(|(named, size)| rustmanifest_schema::ResolvedLayout {
                    named: named.to_owned(),
                    size,
                    align: 8
                })
                .collect(),
            ..rustmanifest_schema::Resolution::default()
        };
        settled.settle();
        settled
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
        let text = "struct User { name: String }\nfn take_name(user: User) -> String { user.name }";
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let settled = Settling::over("src/lib.rs", text)
            .field("user.name", "User", "String")
            .done();
        assert_eq!(insights(&Workspace::resolved(&sources, &settled)).len(), 0);
    }

}
