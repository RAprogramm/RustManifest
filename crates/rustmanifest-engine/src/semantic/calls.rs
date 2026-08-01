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

use std::collections::BTreeMap;

use super::behavior::{Effect, FunctionFacts};

/// Resolved workspace call graph over one behavioral fact slice.
#[derive(Debug)]
pub struct CallGraph {
    /// Callee fact indices per caller fact index.
    edges:      Vec<Vec<usize>>,
    /// Call candidates that resolved to exactly one known function.
    resolved:   u64,
    /// Candidates that were ambiguous or named no known function.
    unresolved: u64
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
}

/// Builds the call graph of `facts` by resolving every call candidate.
#[must_use]
pub fn call_graph(facts: &[FunctionFacts]) -> CallGraph {
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
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); facts.len()];
    let mut resolved = 0u64;
    let mut unresolved = 0u64;
    for (index, fact) in facts.iter().enumerate() {
        for candidate in &fact.calls {
            let matches = candidate.strip_prefix("?::").map_or_else(
                || {
                    if candidate.contains("::") {
                        by_qualified.get(candidate.as_str())
                    } else {
                        by_bare_free.get(candidate.as_str())
                    }
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
                _ => unresolved += 1
            }
        }
        edges[index].sort_unstable();
    }
    CallGraph {
        edges,
        resolved,
        unresolved
    }
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

    #[test]
    fn free_function_calls_resolve_by_bare_name() {
        let all = facts("fn helper() {}\nfn caller() { helper(); }");
        let graph = call_graph(&all);
        assert_eq!(
            graph.callees(index_of(&all, "caller")),
            &[index_of(&all, "helper")]
        );
        assert_eq!(graph.resolved(), 1);
    }

    #[test]
    fn associated_calls_resolve_by_qualified_name() {
        let all =
            facts("struct Board;\nimpl Board { fn reset() {} }\nfn wipe() { Board::reset(); }");
        let graph = call_graph(&all);
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
        let graph = call_graph(&all);
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
        let graph = call_graph(&all);
        assert!(graph.callees(index_of(&all, "run")).is_empty());
        assert_eq!(graph.unresolved(), 1);
    }

    #[test]
    fn effects_propagate_transitively() {
        let all = facts(
            "fn log(text: &str) { println!(\"{text}\"); }\nfn middle(text: &str) { log(text); \
             }\nfn top(text: &str) { middle(text); }"
        );
        let graph = call_graph(&all);
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
        let graph = call_graph(&all);
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "ping")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn external_calls_are_honestly_unresolved() {
        let all = facts("fn read() -> String { std::fs::read_to_string(\"x\").unwrap() }");
        let graph = call_graph(&all);
        assert_eq!(graph.resolved(), 0);
        assert!(graph.unresolved() > 0);
    }
}
