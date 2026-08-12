// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of the call graph: the candidate loop that decides, for
//! every call site, whether it names exactly one known function, several
//! dispatch possibilities, or nothing the workspace holds.

use std::collections::BTreeSet;

use super::{
    context::ResolutionContext,
    dispatch::dispatch_targets,
    graph::CallGraph,
    overlay::apply_overlay,
    reach::linkable,
    rows::{RefusalRow, refusal_row},
    scope::Scopes,
    tables::{name_tables, named as named_of}
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
/// index layer, and it works twice. Inside the candidate loop it
/// settles ambiguity: a candidate matching several linkable functions
/// binds when the index confirms exactly one of them — the uniqueness
/// gate still admits only a single confirmed match, and the edge is
/// marked settled so a trace names both layers. After the loop the
/// remaining overlay edges land additively; an edge the candidates
/// already produced is not recounted, so [`CallGraph::scip_resolved`]
/// states exactly what the index added on its own.
#[must_use]
pub fn call_graph_with(
    facts: &[FunctionFacts],
    context: &ResolutionContext,
    overlay: Option<&[Vec<usize>]>
) -> CallGraph {
    built(facts, context, overlay, None)
}

/// Builds the call graph and reports every refused candidate's row.
///
/// The per-fact rows are the accounting's own answer, not a
/// reconstruction: the rows and the graph's counters pass through the
/// same decision point, so the two cannot disagree.
#[must_use]
pub fn call_graph_rows(
    facts: &[FunctionFacts],
    context: &ResolutionContext
) -> (CallGraph, Vec<Vec<(String, RefusalRow)>>) {
    call_graph_rows_with(facts, context, None)
}

/// Builds the call graph with an optional overlay and reports every
/// refused candidate's row.
///
/// The overlay decides candidates, so a row report taken without it
/// would describe a different graph than the counters count — the rows
/// travel with the same overlay for the same reason they travel
/// through the same decision point.
#[must_use]
pub fn call_graph_rows_with(
    facts: &[FunctionFacts],
    context: &ResolutionContext,
    overlay: Option<&[Vec<usize>]>
) -> (CallGraph, Vec<Vec<(String, RefusalRow)>>) {
    let mut rows: Vec<Vec<(String, RefusalRow)>> = vec![Vec::new(); facts.len()];
    let graph = built(facts, context, overlay, Some(&mut rows));
    (graph, rows)
}

/// The one builder both public forms call: resolves every candidate,
/// books each refusal into its row, and hands the row to the collector
/// where one listens.
fn built(
    facts: &[FunctionFacts],
    context: &ResolutionContext,
    overlay: Option<&[Vec<usize>]>,
    mut rows: Option<&mut Vec<Vec<(String, RefusalRow)>>>
) -> CallGraph {
    let module_segments = &context.segments;
    let tables = name_tables(facts);
    let workspace_owners: BTreeSet<&str> = facts
        .iter()
        .filter_map(|fact| fact.function.rsplit_once("::"))
        .map(|(owner, _)| owner)
        .collect();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); facts.len()];
    let mut tallies = Tallies::default();
    let mut possibilistic: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut settled: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (index, fact) in facts.iter().enumerate() {
        for candidate in &fact.calls {
            let named = named_of(candidate, &tables, module_segments);
            let matches = named
                .as_ref()
                .map(|indices| linkable(context, facts, index, indices));
            if let Some([only]) = matches.as_deref() {
                tallies.resolved += 1;
                if !edges[index].contains(only) {
                    edges[index].push(*only);
                }
                continue;
            }
            if let Some(only) =
                scoped_home(facts, &context.scopes, index, candidate, matches.as_deref())
            {
                tallies.resolved += 1;
                if !edges[index].contains(&only) {
                    edges[index].push(only);
                }
                continue;
            }
            if let (Some(found), Some(over)) = (matches.as_deref(), overlay)
                && found.len() > 1
                && let Some(listed) = over.get(index)
            {
                let mut confirmed = found.iter().copied().filter(|held| listed.contains(held));
                if let (Some(only), None) = (confirmed.next(), confirmed.next()) {
                    tallies.resolved += 1;
                    settled.insert((index, only));
                    if !edges[index].contains(&only) {
                        edges[index].push(only);
                    }
                    continue;
                }
            }
            let targets = candidate.rsplit_once("::").map_or_else(Vec::new, |(o, m)| {
                linkable(
                    context,
                    facts,
                    index,
                    &dispatch_targets(o, m, context, &tables.0)
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
                tallies.resolved += 1;
                continue;
            }
            let row = refusal_row(
                candidate,
                named.is_some(),
                matches.as_deref(),
                &workspace_owners,
                context
            );
            tallies.book(row);
            if let Some(collected) = rows.as_deref_mut() {
                collected[index].push((candidate.clone(), row));
            }
        }
        edges[index].sort_unstable();
    }
    let indexed = overlay.map_or_else(BTreeSet::new, |overlay| {
        apply_overlay(&mut edges, overlay, &|caller, callee| {
            !linkable(context, facts, caller, &[callee]).is_empty()
        })
    });
    tallies.graph(edges, indexed, settled, possibilistic)
}

/// The one definition an unqualified call means by Rust's own scoping,
/// where several workspace functions carry the name: the language
/// resolves an unqualified path in the calling scope before looking
/// wider, so the single definition standing in the caller's own scope
/// is the meant one and not a pick between siblings.
///
/// Two conditions hold it to what the sources prove. The candidate must
/// be written bare — a qualifier names its own scope, and a receiver's
/// method is chosen by type, not by module. And the caller's scope must
/// hold exactly one of the matches, scope meaning the innermost inline
/// module rather than the file: a file's test module and its top level
/// are two scopes, and a call in one of them does not reach a namesake
/// in the other. Nothing here consults the imports, because nothing
/// needs to: an item declared in the calling scope cannot be shadowed —
/// a same-named explicit import in that scope does not compile, and a
/// globbed one loses to the declaration.
fn scoped_home(
    facts: &[FunctionFacts],
    scopes: &Scopes,
    caller: usize,
    candidate: &str,
    matches: Option<&[usize]>
) -> Option<usize> {
    if candidate.contains("::") {
        return None;
    }
    let found = matches?;
    if found.len() < 2 {
        return None;
    }
    let calling = facts.get(caller)?;
    let mut housed = found.iter().copied().filter(|held| {
        facts.get(*held).is_some_and(|fact| {
            fact.source == calling.source
                && scopes.shared(fact.source, fact.range.start, calling.range.start)
        })
    });
    match (housed.next(), housed.next()) {
        (Some(only), None) => Some(only),
        _ => None
    }
}

/// The resolution tallies while the graph is under construction.
#[derive(Default)]
struct Tallies {
    /// Candidates bound to exactly one function, dispatch included.
    resolved:          u64,
    /// Every refused candidate, whatever the row below it lands in.
    unresolved:        u64,
    /// Names the workspace never declares.
    external:          u64,
    /// `?::` names the workspace declares reachably.
    unknown_receiver:  u64,
    /// Names declared somewhere the caller cannot link against.
    unreachable_named: u64,
    /// Names matching several linkable functions.
    ambiguous:         u64,
    /// Names whose declared owner carries no such member.
    owned_unmatched:   u64,
    /// Candidates naming a variant of a workspace enum: values built,
    /// not functions called.
    constructions:     u64,
    /// Candidates whose member the owner carries without a written body.
    granted:           u64
}

impl Tallies {
    /// Books one decided row, so the counters and the per-candidate
    /// report cannot disagree: both are fed by the same decision.
    const fn book(&mut self, row: RefusalRow) {
        self.unresolved += 1;
        match row {
            RefusalRow::Foreign => self.external += 1,
            RefusalRow::UnknownReceiver => self.unknown_receiver += 1,
            RefusalRow::NamedElsewhere => self.unreachable_named += 1,
            RefusalRow::Ambiguous => self.ambiguous += 1,
            RefusalRow::OwnerWithoutMember => self.owned_unmatched += 1,
            RefusalRow::GrantedWithoutBody => self.granted += 1,
            RefusalRow::Construction => self.constructions += 1
        }
    }

    /// Seals the tallies into the finished graph.
    const fn graph(
        self,
        edges: Vec<Vec<usize>>,
        indexed: BTreeSet<(usize, usize)>,
        settled: BTreeSet<(usize, usize)>,
        possibilistic: BTreeSet<(usize, usize)>
    ) -> CallGraph {
        CallGraph {
            edges,
            resolved: self.resolved,
            unresolved: self.unresolved,
            external: self.external,
            unknown_receiver: self.unknown_receiver,
            unreachable_named: self.unreachable_named,
            ambiguous: self.ambiguous,
            owned_unmatched: self.owned_unmatched,
            constructions: self.constructions,
            granted: self.granted,
            indexed,
            settled,
            possibilistic
        }
    }
}
