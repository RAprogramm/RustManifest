// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace call graph: who calls whom, resolved from behavioral call
//! candidates, and effect propagation over it.
//!
//! Resolution is conservative and honest: a candidate produces an edge
//! only when it matches exactly one known function — a qualified
//! `Type::method` by full name, a bare `name` against free functions, an
//! unqualified `?::method` against method names workspace-wide. Ambiguous
//! and external candidates are counted as unresolved, never guessed.
//!
//! [`transitive_effects`] closes the direct per-function effect sets over
//! the graph to a fixpoint: a function that calls a filesystem-touching
//! function transitively touches the filesystem. Cycles (recursion) are
//! handled by the fixpoint iteration.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    behavior::{Effect, FunctionFacts},
    expr::impl_type_name,
    module_overview::module_overview
};
use crate::source::Source;

/// Everything call resolution knows about the workspace beyond the
/// facts themselves.
#[derive(Debug, Default)]
pub struct ResolutionContext {
    /// Module and crate qualifiers, from [`resolution_segments`].
    pub segments:    BTreeSet<String>,
    /// Trait name to the sorted types implementing it, for dynamic
    /// dispatch: a call through `dyn Trait` produces an edge to every
    /// implementor's method.
    pub trait_impls: BTreeMap<String, Vec<String>>,
    /// Default-bodied method name to the extension traits declaring it,
    /// each with its first supertrait — the `trait Ext: Base` pattern
    /// whose blanket impl grants `Ext` to every `Base` implementor.
    pub ext_traits:  BTreeMap<String, Vec<(String, String)>>,
    /// Trait name to its first supertrait, so dispatch on an extension
    /// trait's receiver falls through to the supertrait's implementors.
    pub supertraits: BTreeMap<String, String>
}

/// Builds the full resolution context over `sources`.
#[must_use]
pub fn resolution_context(sources: &[Source]) -> ResolutionContext {
    let mut trait_impls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ext_traits: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut supertraits: BTreeMap<String, String> = BTreeMap::new();
    for source in sources {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Impl(block) => {
                    let (Some((path, _)), Some(implementor)) =
                        (&block.trait_, impl_type_name(block))
                    else {
                        continue;
                    };
                    let Some(trait_name) = path.segments.last().map(|s| s.ident.to_string())
                    else {
                        continue;
                    };
                    let implementors = trait_impls.entry(trait_name).or_default();
                    if !implementors.contains(&implementor) {
                        implementors.push(implementor);
                    }
                }
                syn::Item::Trait(definition) => {
                    let Some(base) = first_supertrait(definition) else {
                        continue;
                    };
                    let ext = definition.ident.to_string();
                    supertraits
                        .entry(ext.clone())
                        .or_insert_with(|| base.clone());
                    for trait_item in &definition.items {
                        if let syn::TraitItem::Fn(method) = trait_item
                            && method.default.is_some()
                        {
                            let entry = (ext.clone(), base.clone());
                            let entries =
                                ext_traits.entry(method.sig.ident.to_string()).or_default();
                            if !entries.contains(&entry) {
                                entries.push(entry);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for implementors in trait_impls.values_mut() {
        implementors.sort_unstable();
    }
    for entries in ext_traits.values_mut() {
        entries.sort_unstable();
    }
    ResolutionContext {
        segments: resolution_segments(sources),
        trait_impls,
        ext_traits,
        supertraits
    }
}

/// Last segment of a trait definition's first supertrait bound.
fn first_supertrait(definition: &syn::ItemTrait) -> Option<String> {
    definition.supertraits.iter().find_map(|bound| {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            trait_bound
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
        } else {
            None
        }
    })
}

/// Builds the qualifier set for call resolution over `sources`.
///
/// The set holds the last segment of every module path plus the name of
/// every workspace crate, derived from the `<crate>/src/` path
/// convention with `-` normalized to `_`. A candidate qualified by any
/// of these resolves like a module qualifier — to a unique bare free
/// function.
#[must_use]
pub fn resolution_segments(sources: &[Source]) -> BTreeSet<String> {
    let mut segments: BTreeSet<String> = module_overview(sources)
        .modules()
        .iter()
        .filter_map(|module| module.path.rsplit("::").next())
        .map(ToOwned::to_owned)
        .collect();
    for source in sources {
        let mut components = source.path().components().peekable();
        let mut previous: Option<String> = None;
        while let Some(component) = components.next() {
            let name = component.as_os_str().to_string_lossy();
            if name == "src"
                && components.peek().is_some()
                && let Some(krate) = previous.take()
            {
                segments.insert(krate.replace('-', "_"));
                break;
            }
            previous = Some(name.into_owned());
        }
    }
    segments
}

/// Resolved workspace call graph over one behavioral fact slice.
#[derive(Debug)]
pub struct CallGraph {
    /// Callee fact indices per caller fact index.
    edges:            Vec<Vec<usize>>,
    /// Call candidates that resolved to exactly one known function.
    resolved:         u64,
    /// Candidates that were ambiguous or named no known function.
    unresolved:       u64,
    /// Unmatched candidates provably outside the workspace: the owner
    /// names no workspace type or module, or no workspace function
    /// carries the called name at all — external by elimination.
    external:         u64,
    /// Unmatched `?::method` candidates whose method name the workspace
    /// does define — a typed receiver would settle them, so they are the
    /// engine's own understanding gap.
    unknown_receiver: u64,
    /// Edges added by the SCIP overlay that the syntactic candidates
    /// alone had not produced — resolution owed to the index.
    scip_resolved:    u64,
    /// Edges born from dispatch over several implementors — the callee
    /// is one possibility among siblings, not a certainty.
    possibilistic:    BTreeSet<(usize, usize)>
}

impl CallGraph {
    /// Callee fact indices of the function at `index`, deduplicated and
    /// sorted.
    #[must_use]
    pub fn callees(&self, index: usize) -> &[usize] {
        self.edges.get(index).map_or(&[], Vec::as_slice)
    }

    /// Call candidates that resolved to exactly one known function.
    #[must_use]
    pub const fn resolved(&self) -> u64 {
        self.resolved
    }

    /// Candidates that were ambiguous or named no known function.
    #[must_use]
    pub const fn unresolved(&self) -> u64 {
        self.unresolved
    }

    /// Unmatched candidates provably attributable to external code.
    #[must_use]
    pub const fn external(&self) -> u64 {
        self.external
    }

    /// Unmatched candidates a typed receiver would settle in-workspace.
    #[must_use]
    pub const fn unknown_receiver(&self) -> u64 {
        self.unknown_receiver
    }

    /// Edges the SCIP overlay added beyond the syntactic candidates.
    #[must_use]
    pub const fn scip_resolved(&self) -> u64 {
        self.scip_resolved
    }

    /// Returns whether the edge came from dispatch over several
    /// implementors — a possibility, not a certainty.
    #[must_use]
    pub fn possibilistic(&self, from: usize, to: usize) -> bool {
        self.possibilistic.contains(&(from, to))
    }
}

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
            match matches {
                Some(indices) if indices.len() == 1 => {
                    resolved += 1;
                    if !edges[index].contains(&indices[0]) {
                        edges[index].push(indices[0]);
                    }
                }
                _ => {
                    let targets = candidate.rsplit_once("::").map_or_else(Vec::new, |(o, m)| {
                        dispatch_targets(o, m, context, &by_qualified)
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
                        if by_method.contains_key(method) {
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
            }
        }
        edges[index].sort_unstable();
    }
    let scip_resolved = overlay.map_or(0, |overlay| apply_overlay(&mut edges, overlay));
    CallGraph {
        edges,
        resolved,
        unresolved,
        external,
        unknown_receiver,
        scip_resolved,
        possibilistic
    }
}

/// Resolves an unmatched `Owner::method` candidate through the trait
/// machinery: dynamic dispatch over the owner's implementors when the
/// owner names a trait — falling through extension traits to their
/// supertrait's implementors — or the unique extension-trait default
/// granting `method` to a concrete owner.
fn dispatch_targets(
    owner: &str,
    method: &str,
    context: &ResolutionContext,
    by_qualified: &BTreeMap<&str, Vec<usize>>
) -> Vec<usize> {
    let implementors = implementors_of(owner, context);
    if let Some(implementors) = implementors {
        let mut targets = Vec::new();
        for implementor in implementors {
            let qualified = format!("{implementor}::{method}");
            if let Some([only]) = by_qualified.get(qualified.as_str()).map(Vec::as_slice) {
                targets.push(*only);
            }
        }
        if !targets.is_empty() {
            return targets;
        }
    }
    ext_target(owner, method, context, by_qualified)
        .map(|target| vec![target])
        .unwrap_or_default()
}

/// Implementors of a trait, seen through the extension-trait pattern:
/// a trait with no direct impls falls through its supertrait chain,
/// bounded so a cycle cannot loop.
fn implementors_of<'context>(
    owner: &str,
    context: &'context ResolutionContext
) -> Option<&'context Vec<String>> {
    let mut current = owner;
    for _ in 0..4 {
        if let Some(implementors) = context.trait_impls.get(current) {
            return Some(implementors);
        }
        current = context.supertraits.get(current)?;
    }
    None
}

/// Resolves an unmatched `Owner::method` candidate to the unique
/// extension-trait default that grants `method` to `Owner`: the owner
/// must implement the extension's supertrait, and the extension's
/// default must exist as exactly one fact. More than one granting
/// extension is ambiguity, and ambiguity stays unresolved.
fn ext_target(
    owner: &str,
    method: &str,
    context: &ResolutionContext,
    by_qualified: &BTreeMap<&str, Vec<usize>>
) -> Option<usize> {
    let entries = context.ext_traits.get(method)?;
    let mut targets = entries.iter().filter_map(|(ext, base)| {
        let grants = context
            .trait_impls
            .get(base)
            .is_some_and(|implementors| implementors.iter().any(|name| name == owner));
        if !grants {
            return None;
        }
        let qualified = format!("{ext}::{method}");
        match by_qualified.get(qualified.as_str()).map(Vec::as_slice) {
            Some([only]) => Some(*only),
            _ => None
        }
    });
    let first = targets.next()?;
    targets.next().is_none().then_some(first)
}

/// Resolution tables of one fact slice: qualified names, bare free
/// functions, and method names, each to the fact indices carrying them.
type NameTables<'facts> = (
    BTreeMap<&'facts str, Vec<usize>>,
    BTreeMap<&'facts str, Vec<usize>>,
    BTreeMap<&'facts str, Vec<usize>>
);

/// Builds the resolution tables the candidate loop and the overlay
/// share.
fn name_tables(facts: &[FunctionFacts]) -> NameTables<'_> {
    let mut by_qualified: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_bare_free: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_method: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        by_qualified.entry(&fact.function).or_default().push(index);
        match fact.function.rsplit_once("::") {
            None => by_bare_free.entry(&fact.function).or_default().push(index),
            Some((_, method)) => by_method.entry(method).or_default().push(index)
        }
    }
    (by_qualified, by_bare_free, by_method)
}

/// Applies overlay callee indices onto the edge lists and returns how
/// many edges the overlay added beyond the ones already present.
fn apply_overlay(edges: &mut [Vec<usize>], overlay: &[Vec<usize>]) -> u64 {
    let mut added = 0u64;
    for (index, callees) in overlay.iter().enumerate().take(edges.len()) {
        for &callee in callees {
            if !edges[index].contains(&callee) {
                edges[index].push(callee);
                added += 1;
            }
        }
        edges[index].sort_unstable();
    }
    added
}

/// Closes per-function effect sets over the call graph to a fixpoint.
///
/// The result at position `i` is the direct effects of `facts[i]` united
/// with the effects of everything it transitively calls, sorted and
/// deduplicated.
#[must_use]
pub fn transitive_effects(facts: &[FunctionFacts], graph: &CallGraph) -> Vec<Vec<Effect>> {
    let mut sets: Vec<Vec<Effect>> = facts.iter().map(|fact| fact.effects.clone()).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for caller in 0..sets.len() {
            for &callee in graph.callees(caller) {
                let additions: Vec<Effect> = sets[callee]
                    .iter()
                    .filter(|effect| !sets[caller].contains(effect))
                    .copied()
                    .collect();
                if !additions.is_empty() {
                    sets[caller].extend(additions);
                    changed = true;
                }
            }
        }
    }
    for set in &mut sets {
        set.sort();
        set.dedup();
    }
    sets
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

    use super::{call_graph, transitive_effects};
    use crate::{
        semantic::behavior::{Effect, behavior},
        source::Source
    };

    fn facts(text: &str) -> Vec<crate::semantic::behavior::FunctionFacts> {
        behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
    }

    fn index_of(all: &[crate::semantic::behavior::FunctionFacts], name: &str) -> usize {
        all.iter().position(|fact| fact.function == name).unwrap()
    }

    fn context_graph(
        text: &str
    ) -> (
        Vec<crate::semantic::behavior::FunctionFacts>,
        super::CallGraph
    ) {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let graph = call_graph(&facts, &super::resolution_context(&sources));
        (facts, graph)
    }

    #[test]
    fn generic_bound_parameters_dispatch_over_implementors() {
        let (all, graph) = context_graph(
            "trait Sink { fn write(&self); }\nstruct Disk;\nimpl Sink for Disk { fn write(&self) \
             {} }\nstruct Wire;\nimpl Sink for Wire { fn write(&self) {} }\nfn drain<S: \
             Sink>(sink: S) { sink.write(); }"
        );
        let callees = graph.callees(index_of(&all, "drain"));
        assert!(callees.contains(&index_of(&all, "Disk::write")));
        assert!(callees.contains(&index_of(&all, "Wire::write")));
    }

    #[test]
    fn ext_trait_defaults_dispatch_through_the_supertrait_bound() {
        let (all, graph) = context_graph(
            "trait Base { fn poll(&self); }\nstruct Conn;\nimpl Base for Conn { fn poll(&self) { \
             std::net::TcpStream::connect(\"h\").ok(); } }\npub trait Ext: Base {\n    fn \
             write_all(&self) { self.poll(); }\n}\nstruct Plain;\nfn go(conn: Conn, other: \
             Plain) { conn.write_all(); other.write_all(); }"
        );
        let callees = graph.callees(index_of(&all, "go"));
        assert!(callees.contains(&index_of(&all, "Ext::write_all")));
        let effects = super::transitive_effects(&all, &graph);
        assert!(effects[index_of(&all, "go")].contains(&Effect::Network));
    }

    #[test]
    fn scip_overlay_adds_only_edges_the_candidates_missed() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "struct Wallet;\nimpl Wallet { fn credit(&self) {} }\nfn helper() {}\nfn caller() { \
             helper(); }"
                .to_owned()
        )];
        let all = behavior(&sources);
        let context = super::resolution_context(&sources);
        let caller = index_of(&all, "caller");
        let mut overlay = vec![Vec::new(); all.len()];
        overlay[caller] = vec![index_of(&all, "Wallet::credit"), index_of(&all, "helper")];
        let graph = super::call_graph_with(&all, &context, Some(&overlay));
        let callees = graph.callees(caller);
        assert!(callees.contains(&index_of(&all, "Wallet::credit")));
        assert!(callees.contains(&index_of(&all, "helper")));
        assert_eq!(graph.scip_resolved(), 1);
        let plain = call_graph(&all, &context);
        assert_eq!(plain.scip_resolved(), 0);
    }

    #[test]
    fn trait_object_calls_dispatch_to_every_implementor() {
        let (all, graph) = context_graph(
            "trait Analyzer { fn analyze(&self); }\nstruct A;\nimpl Analyzer for A { fn \
             analyze(&self) {} }\nstruct B;\nimpl Analyzer for B { fn analyze(&self) {} \
             }\nstruct Hub { analyzers: Vec<Box<dyn Analyzer>> }\nimpl Hub { fn run(&self) { \
             for analyzer in &self.analyzers { analyzer.analyze(); } } }"
        );
        let callees = graph.callees(index_of(&all, "Hub::run"));
        assert!(callees.contains(&index_of(&all, "A::analyze")));
        assert!(callees.contains(&index_of(&all, "B::analyze")));
    }

    #[test]
    fn dyn_reference_parameters_dispatch_too() {
        let (all, graph) = context_graph(
            "trait Sink { fn write(&self); }\nstruct File;\nimpl Sink for File { fn write(&self) \
             {} }\nstruct Net;\nimpl Sink for Net { fn write(&self) {} }\nfn drain(sink: &dyn \
             Sink) { sink.write(); }"
        );
        let callees = graph.callees(index_of(&all, "drain"));
        assert!(callees.contains(&index_of(&all, "File::write")));
        assert!(callees.contains(&index_of(&all, "Net::write")));
    }

    #[test]
    fn free_function_calls_resolve_by_bare_name() {
        let all = facts("fn helper() {}\nfn caller() { helper(); }");
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "caller")),
            &[index_of(&all, "helper")]
        );
        assert_eq!(graph.resolved(), 1);
    }

    #[test]
    fn field_receivers_resolve_through_struct_definitions() {
        let all = facts(
            "struct Engine;\nimpl Engine { fn start(&self) {} }\nstruct Car { engine: Engine \
             }\nimpl Car { fn drive(&self) { self.engine.start(); } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "Car::drive")),
            &[index_of(&all, "Engine::start")]
        );
    }

    #[test]
    fn loop_variables_type_through_collection_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) { for item in items { item.ping(); } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn closure_parameters_type_through_iterated_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: Vec<S>) { items.iter().for_each(|item| item.ping()); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn indexed_receivers_type_through_collection_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) { items[0].ping(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn field_collections_type_their_iterated_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nstruct Holder { items: Vec<S> }\nimpl Holder { fn go(&self) { for item in \
             &self.items { item.ping(); } } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "Holder::go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn unwrapped_external_constructors_attribute_calls_outside() {
        let all = facts(
            "struct S;\nimpl S { fn path(&self) {} }\nstruct T;\nimpl T { fn path(&self) {} \
             }\nfn go() { let dir = TempDir::new().unwrap(); dir.path(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.unknown_receiver(), 0);
        assert_eq!(graph.external(), 3);
    }

    #[test]
    fn builder_chains_type_through_return_tables() {
        let all = facts(
            "struct R;\nimpl R { fn new() -> Self { R } fn with_color(self) -> Self { self } fn \
             render(&self) {} }\nstruct Q;\nimpl Q { fn render(&self) {} }\nfn go() { let out = \
             R::new().with_color(); out.render(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "R::render"))
        );
    }

    #[test]
    fn enumerate_tuples_bind_the_element_half() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) { for (index, item) in items.iter().enumerate() { \
             item.ping(); } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "S::ping"))
        );
    }

    #[test]
    fn optional_accessors_yield_the_element_type() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) -> Option<()> { items.get(0)?.ping(); Some(()) }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "S::ping"))
        );
    }

    #[test]
    fn known_external_constructors_attribute_calls_outside() {
        let all = facts(
            "struct S;\nimpl S { fn path(&self) {} }\nstruct T;\nimpl T { fn path(&self) {} \
             }\nfn go() { let dir = tempfile::tempdir().unwrap(); dir.path(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.unknown_receiver(), 0);
    }

    #[test]
    fn unmatched_candidates_split_into_external_and_unknown_receiver() {
        let all = facts(
            "fn go() { let text = String::from(\"x\"); opaque().push(text); opaque().ping(); \
             }\nstruct A;\nimpl A { fn ping(&self) {} }\nstruct B;\nimpl B { fn ping(&self) {} \
             }\nfn miss() { A::gone(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.resolved(), 0);
        assert_eq!(graph.external(), 3);
        assert_eq!(graph.unknown_receiver(), 1);
        assert_eq!(graph.unresolved(), 5);
    }

    #[test]
    fn associated_calls_resolve_by_qualified_name() {
        let all =
            facts("struct Board;\nimpl Board { fn reset() {} }\nfn wipe() { Board::reset(); }");
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "wipe")),
            &[index_of(&all, "Board::reset")]
        );
    }

    #[test]
    fn method_calls_resolve_through_typed_receivers() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn balance(&self) -> i64 { \
             self.balance } }\nfn audit(wallet: &Wallet) -> i64 { wallet.balance() }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "audit")),
            &[index_of(&all, "Wallet::balance")]
        );
    }

    #[test]
    fn ambiguous_bare_names_stay_unresolved() {
        let all = facts(
            "struct A;\nimpl A { fn go(&self) {} }\nstruct B;\nimpl B { fn go(&self) {} }\nfn \
             run(x: u32) { x.go(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(graph.callees(index_of(&all, "run")).is_empty());
        assert_eq!(graph.unresolved(), 1);
    }

    #[test]
    fn effects_propagate_transitively() {
        let all = facts(
            "fn log(text: &str) { println!(\"{text}\"); }\nfn middle(text: &str) { log(text); \
             }\nfn top(text: &str) { middle(text); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "top")], vec![Effect::ConsoleIo]);
        assert_eq!(effects[index_of(&all, "middle")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn recursion_reaches_a_fixpoint() {
        let all = facts(
            "fn ping(n: u32) { if n > 0 { pong(n - 1); } }\nfn pong(n: u32) { println!(\"{n}\"); \
             ping(n); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "ping")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn external_calls_are_honestly_unresolved() {
        let all = facts("fn read() -> String { std::fs::read_to_string(\"x\").unwrap() }");
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.resolved(), 0);
        assert!(graph.unresolved() > 0);
    }

    #[test]
    fn cross_crate_calls_resolve_through_crate_names() {
        let sources = [
            Source::new(
                PathBuf::from("crates/rusty-core/src/lib.rs"),
                "pub fn helper() { println!(\"x\"); }".to_owned()
            ),
            Source::new(
                PathBuf::from("crates/rusty-app/src/lib.rs"),
                "pub fn run() { rusty_core::helper(); }".to_owned()
            )
        ];
        let all = crate::semantic::behavior(&sources);
        let graph = call_graph(&all, &super::resolution_context(&sources));
        assert_eq!(
            graph.callees(index_of(&all, "run")),
            &[index_of(&all, "helper")]
        );
    }

    #[test]
    fn factory_typed_receivers_resolve_method_calls() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn balance(&self) -> i64 { \
             self.balance } }\nfn make_wallet() -> Wallet { Wallet { balance: 0 } }\nfn \
             audit() -> i64 { let w = make_wallet(); w.balance() }\nfn peek() -> i64 { \
             make_wallet().balance() }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        let target = index_of(&all, "Wallet::balance");
        assert!(graph.callees(index_of(&all, "audit")).contains(&target));
        assert!(graph.callees(index_of(&all, "peek")).contains(&target));
    }
}
