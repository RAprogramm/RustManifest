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
    rows::RefusalRow,
    tables::name_tables
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

/// Books a candidate a resolver already settled, and answers whether it
/// did.
///
/// A candidate reaching a body of this workspace is resolved; one reaching
/// anything else is a call out of the workspace. Both go through the same
/// booking as every read candidate, so the counters and the per-candidate
/// report cannot disagree about them either.
fn settled_already(
    asked: (usize, &String),
    settled: (&Asked, &Asked),
    tallies: &mut Tallies,
    rows: Option<&mut Vec<Vec<(String, RefusalRow)>>>
) -> bool {
    let (index, candidate) = asked;
    let (bound, outside) = settled;
    let held = (index, candidate.clone());
    if bound.contains(&held) {
        tallies.resolved += 1;
        return true;
    }
    if !outside.contains(&held) {
        return false;
    }
    tallies.book(RefusalRow::Foreign);
    if let Some(rows) = rows
        && let Some(row) = rows.get_mut(index)
    {
        row.push((candidate.clone(), RefusalRow::Foreign));
    }
    true
}

/// One candidate of one caller: the pair a settled site answers for.
type Asked = BTreeSet<(usize, String)>;

/// Binds every call site whose reached body a resolver named, and answers
/// which candidates that settled.
///
/// A site the resolver settled needs no candidate reading at all, and that
/// holds whichever way it landed: a call reaching a body this workspace
/// declares becomes an edge, and one reaching anything else is a call out
/// of the workspace. The reading below cannot tell those apart — it binds
/// by written name, so a call into a dependency lands on a same-named body
/// here and the graph gains an edge no build has.
fn bound_by_identity(
    facts: &[FunctionFacts],
    context: &ResolutionContext,
    bodies: &std::collections::BTreeMap<&str, usize>,
    edges: &mut [Vec<usize>]
) -> (Asked, Asked) {
    let mut bound = BTreeSet::new();
    let mut outside = BTreeSet::new();
    for (index, fact) in facts.iter().enumerate() {
        for site in &fact.call_sites {
            let Some(reached) = context
                .files
                .get(fact.source)
                .and_then(|file| context.identity.reached(file, site.site))
            else {
                continue;
            };
            let Some(&target) = bodies.get(reached) else {
                outside.insert((index, site.candidate.clone()));
                continue;
            };
            bound.insert((index, site.candidate.clone()));
            if !edges[index].contains(&target) {
                edges[index].push(target);
            }
        }
    }
    outside.retain(|held| !bound.contains(held));
    (bound, outside)
}

/// Every declared body by the full path a resolver settled for it.
///
/// The pairing a written name cannot make: a fact holds where a body is
/// written, and the resolution holds what the body at that place is
/// called.
fn declared_bodies<'a>(
    facts: &[FunctionFacts],
    context: &'a ResolutionContext
) -> std::collections::BTreeMap<&'a str, usize> {
    let mut bodies = std::collections::BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        let Some(named) = context
            .files
            .get(fact.source)
            .and_then(|file| context.identity.declared_as(file, fact.range))
        else {
            continue;
        };
        bodies.entry(named).or_insert(index);
    }
    bodies
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
    let tables = name_tables(facts);
    let bodies = declared_bodies(facts, context);
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); facts.len()];
    let mut tallies = Tallies::default();
    let mut possibilistic: BTreeSet<(usize, usize)> = BTreeSet::new();
    let settled: BTreeSet<(usize, usize)> = BTreeSet::new();
    let (by_identity, outside) = bound_by_identity(facts, context, &bodies, &mut edges);
    for (index, fact) in facts.iter().enumerate() {
        for candidate in &fact.calls {
            if settled_already(
                (index, candidate),
                (&by_identity, &outside),
                &mut tallies,
                rows.as_deref_mut()
            ) {
                continue;
            }
            let targets = candidate.rsplit_once("::").map_or_else(Vec::new, |(owner, method)| {
                linkable(
                    context,
                    facts,
                    index,
                    &dispatch_targets(owner, method, context, &tables.0)
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
            tallies.book(RefusalRow::Unsettled);
            if let Some(collected) = rows.as_deref_mut() {
                collected[index].push((candidate.clone(), RefusalRow::Unsettled));
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
    granted:           u64,
    /// Candidates no resolver settled a reached body for.
    unsettled:         u64
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
            RefusalRow::Unsettled => self.unsettled += 1,
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
