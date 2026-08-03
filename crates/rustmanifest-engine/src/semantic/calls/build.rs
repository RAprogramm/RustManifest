// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of the call graph: the candidate loop that decides, for
//! every call site, whether it names exactly one known function, several
//! dispatch possibilities, or nothing the workspace holds.

use std::collections::BTreeSet;

use super::{
    context::ResolutionContext, dispatch::dispatch_targets, graph::CallGraph,
    overlay::apply_overlay, reach::linkable, tables::name_tables
};
use crate::semantic::behavior::FunctionFacts;

/// Builds the call graph of `facts` by resolving every call candidate.
///
/// `context.segments` lists module and crate qualifiers; a qualified
/// candidate whose qualifier names one falls back to a unique bare free
/// function (`crate::io::persist()` → `persist`). A candidate whose
/// owner names a trait in `context.trait_impls` produces a dispatch
/// edge to every implementor's method — the honest over-approximation
/// of dynamic dispatch.
#[must_use]
pub fn call_graph(facts: &[FunctionFacts], context: &ResolutionContext) -> CallGraph {
    call_graph_with(facts, context, None)
}

/// Builds the call graph with an optional SCIP overlay.
///
/// The overlay holds per-fact callee fact indices already bound by the
/// index layer, applied after the syntactic candidates. An edge the
/// candidates already produced is not recounted, so
/// [`CallGraph::scip_resolved`] states exactly what the index added.
#[must_use]
pub fn call_graph_with(
    facts: &[FunctionFacts],
    context: &ResolutionContext,
    overlay: Option<&[Vec<usize>]>
) -> CallGraph {
    let module_segments = &context.segments;
    let (by_qualified, by_bare_free, by_method) = name_tables(facts);
    let workspace_owners: BTreeSet<&str> = facts
        .iter()
        .filter_map(|fact| fact.function.rsplit_once("::"))
        .map(|(owner, _)| owner)
        .collect();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); facts.len()];
    let mut resolved = 0u64;
    let mut unresolved = 0u64;
    let mut external = 0u64;
    let mut unknown_receiver = 0u64;
    let mut possibilistic: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (index, fact) in facts.iter().enumerate() {
        for candidate in &fact.calls {
            let matches = candidate.strip_prefix("?::").map_or_else(
                || {
                    candidate.rsplit_once("::").map_or_else(
                        || by_bare_free.get(candidate.as_str()),
                        |(qualifier, bare)| {
                            by_qualified.get(candidate.as_str()).or_else(|| {
                                module_segments
                                    .contains(qualifier)
                                    .then(|| by_bare_free.get(bare))
                                    .flatten()
                            })
                        }
                    )
                },
                |method| by_method.get(method)
            );
            let matches = matches.map(|indices| linkable(context, facts, index, indices));
            if let Some([only]) = matches.as_deref() {
                resolved += 1;
                if !edges[index].contains(only) {
                    edges[index].push(*only);
                }
                continue;
            }
            let targets = candidate.rsplit_once("::").map_or_else(Vec::new, |(o, m)| {
                linkable(
                    context,
                    facts,
                    index,
                    &dispatch_targets(o, m, context, &by_qualified)
                )
            });
            if !targets.is_empty() {
                let ambiguous = targets.len() > 1;
                for target in targets {
                    if ambiguous {
                        possibilistic.insert((index, target));
                    }
                    if !edges[index].contains(&target) {
                        edges[index].push(target);
                    }
                }
                resolved += 1;
                continue;
            }
            unresolved += 1;
            if let Some(method) = candidate.strip_prefix("?::") {
                let reachable = by_method
                    .get(method)
                    .is_some_and(|indices| !linkable(context, facts, index, indices).is_empty());
                if reachable {
                    unknown_receiver += 1;
                } else {
                    external += 1;
                }
            } else if candidate.rsplit_once("::").map_or_else(
                || !by_bare_free.contains_key(candidate.as_str()),
                |(owner, _)| {
                    !workspace_owners.contains(owner)
                        && !module_segments.contains(owner)
                        && !context.trait_impls.contains_key(owner)
                }
            ) {
                external += 1;
            }
        }
        edges[index].sort_unstable();
    }
    let indexed = overlay.map_or_else(BTreeSet::new, |overlay| {
        apply_overlay(&mut edges, overlay, &|caller, callee| {
            !linkable(context, facts, caller, &[callee]).is_empty()
        })
    });
    CallGraph {
        edges,
        resolved,
        unresolved,
        external,
        unknown_receiver,
        indexed,
        possibilistic
    }
}
