// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which functions sit in one recursion cycle, and in what order their
//! summaries can be settled.
//!
//! Condensation over the call graph, emitting each group only after every
//! group it reaches. That order is callee-first by construction, so one
//! pass settles every summary and no fixpoint is needed. A callee inside
//! the caller's own group is the one case a single pass cannot settle,
//! and it is refused by name rather than iterated toward.

use crate::semantic::calls::CallGraph;

/// Which group each function belongs to, and the order the groups can be
/// settled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Components {
    /// Group of every function, by fact index.
    group: Vec<usize>,
    /// Fact indices in an order where a callee precedes its caller,
    /// except inside one group.
    order: Vec<usize>
}

impl Components {
    /// The group a function belongs to.
    pub(super) fn group_of(&self, fact: usize) -> usize {
        self.group.get(fact).copied().unwrap_or(usize::MAX)
    }

    /// Whether two functions sit in one recursion cycle.
    pub(super) fn together(&self, one: usize, other: usize) -> bool {
        self.group_of(one) == self.group_of(other)
    }

    /// Fact indices in settling order.
    pub(super) fn order(&self) -> &[usize] {
        &self.order
    }
}

/// Groups every function of `graph` into recursion cycles and orders them
/// so a callee is settled before its caller.
pub(super) fn components(graph: &CallGraph, count: usize) -> Components {
    let mut walk = Walk {
        graph,
        index: vec![usize::MAX; count],
        low: vec![0; count],
        stack: Vec::new(),
        on_stack: vec![false; count],
        next: 0,
        group: vec![usize::MAX; count],
        order: Vec::new(),
        groups: 0
    };
    for fact in 0..count {
        if walk.index[fact] == usize::MAX {
            walk.visit(fact);
        }
    }
    Components {
        group: walk.group,
        order: walk.order
    }
}

/// State of one condensation walk.
struct Walk<'graph> {
    /// The graph being condensed.
    graph:    &'graph CallGraph,
    /// Discovery number of every function.
    index:    Vec<usize>,
    /// Lowest discovery number reachable from every function.
    low:      Vec<usize>,
    /// Functions whose group is not settled yet.
    stack:    Vec<usize>,
    /// Whether a function is on the stack.
    on_stack: Vec<bool>,
    /// Next discovery number.
    next:     usize,
    /// Group of every settled function.
    group:    Vec<usize>,
    /// Functions in settling order.
    order:    Vec<usize>,
    /// How many groups are settled.
    groups:   usize
}

impl Walk<'_> {
    /// Visits one function, settling every group it reaches first.
    ///
    /// Written as an explicit stack rather than as recursion: a deep call
    /// graph would otherwise decide how deep the analysis can look by
    /// exhausting the machine's own stack, which is a limit of the run
    /// and not of the source.
    fn visit(&mut self, root: usize) {
        let mut pending = vec![(root, 0usize)];
        self.enter(root);
        while let Some((fact, edge)) = pending.pop() {
            let callees = self.graph.callees(fact);
            if edge < callees.len() {
                let callee = callees[edge];
                pending.push((fact, edge + 1));
                if self.index[callee] == usize::MAX {
                    self.enter(callee);
                    pending.push((callee, 0));
                } else if self.on_stack[callee] {
                    self.low[fact] = self.low[fact].min(self.index[callee]);
                }
                continue;
            }
            if self.low[fact] == self.index[fact] {
                self.settle(fact);
            }
            if let Some(&(caller, _)) = pending.last() {
                self.low[caller] = self.low[caller].min(self.low[fact]);
            }
        }
    }

    /// Numbers one function and puts it on the stack.
    fn enter(&mut self, fact: usize) {
        self.index[fact] = self.next;
        self.low[fact] = self.next;
        self.next += 1;
        self.stack.push(fact);
        self.on_stack[fact] = true;
    }

    /// Closes the group rooted at one function.
    fn settle(&mut self, root: usize) {
        while let Some(member) = self.stack.pop() {
            self.on_stack[member] = false;
            self.group[member] = self.groups;
            self.order.push(member);
            if member == root {
                break;
            }
        }
        self.groups += 1;
    }
}
