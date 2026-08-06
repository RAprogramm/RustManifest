// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which names one body writes exactly once.
//!
//! A name written once stands for one value, and an origin read off it is
//! the origin of that value. A name written twice stands for whichever of
//! them reaches the reading position, which is a question of scopes and of
//! iteration order that this tier does not resolve — so every reading of
//! such a name refuses by name rather than picking the textually nearest
//! one.
//!
//! Scopes are counted as scopes. A name is written more than once only
//! where one reading could see two of its writings: twice in the same
//! scope, or once in a scope and once in a scope enclosing it. Two
//! sibling scopes — two arms of a `match`, the halves of an `if`, two
//! blocks written one after the other — each bind their own name, and no
//! position sees both, so siblings contribute the largest count any one
//! of them reaches rather than their sum.
//!
//! Counting the whole body flat called every such name twice-written and
//! refused values the source settles beyond doubt: an error bound in one
//! construct and an error bound in the next are two names, not one name
//! written twice.
//!
//! Three ways of writing a name are counted, and all three are needed.
//! Patterns alone would miss assignment entirely: the field-write reading
//! handles only a field expression, so a bare assignment records nothing
//! on any channel, and a census built on patterns alone would call an
//! accumulator singly written and state its first value as the value every
//! later reading sees.

use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;

use crate::semantic::expr::compound_assignment;

/// Which names one body writes, and which of them it writes once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::semantic::behavior) struct NameCensus {
    /// Every name the body writes, by binding or by assignment.
    pub(in crate::semantic::behavior) written: BTreeSet<String>,
    /// The names it writes exactly once.
    pub(in crate::semantic::behavior) sole:    BTreeSet<String>
}

/// Counts how often `block` writes each name, along the deepest scope
/// path any reading could stand on.
pub(in crate::semantic::behavior) fn name_census(block: &syn::Block) -> NameCensus {
    let counts = scope_counts(|census| census.visit_block(block));
    let mut written = BTreeSet::new();
    let mut sole = BTreeSet::new();
    for (name, count) in counts {
        if count == 1 {
            sole.insert(name.clone());
        }
        written.insert(name);
    }
    NameCensus {
        written,
        sole
    }
}

/// Counts the writings of one scope: its own, plus the largest count any
/// single child scope reaches.
fn scope_counts(visit: impl FnOnce(&mut Census)) -> BTreeMap<String, u32> {
    let mut census = Census {
        counts:   BTreeMap::new(),
        children: Vec::new()
    };
    visit(&mut census);
    let mut counts = census.counts;
    let mut deepest: BTreeMap<String, u32> = BTreeMap::new();
    for child in census.children {
        for (name, count) in child {
            let seen = deepest.entry(name).or_insert(0);
            *seen = (*seen).max(count);
        }
    }
    for (name, count) in deepest {
        *counts.entry(name).or_insert(0) += count;
    }
    counts
}

/// Walk counting how many times each name is written in one scope.
struct Census {
    /// How often each name is bound or assigned in this scope itself.
    counts:   BTreeMap<String, u32>,
    /// What each scope written inside this one counted.
    children: Vec<BTreeMap<String, u32>>
}

impl Census {
    /// Counts one writing of `name`.
    fn note(&mut self, name: String) {
        *self.counts.entry(name).or_insert(0) += 1;
    }

    /// Counts a bare name standing as the target of an assignment.
    fn note_target(&mut self, target: &syn::Expr) {
        if let syn::Expr::Path(path) = target
            && let Some(name) = path.path.get_ident()
        {
            self.note(name.to_string());
        }
    }
}

impl<'ast> Visit<'ast> for Census {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        let counted = scope_counts(|census| syn::visit::visit_block(census, block));
        self.children.push(counted);
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        let counted = scope_counts(|census| syn::visit::visit_arm(census, arm));
        self.children.push(counted);
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        let counted = scope_counts(|census| syn::visit::visit_expr_closure(census, closure));
        self.children.push(counted);
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        let taken = scope_counts(|census| {
            census.visit_expr(&conditional.cond);
            syn::visit::visit_block(census, &conditional.then_branch);
        });
        self.children.push(taken);
        if let Some((_, otherwise)) = &conditional.else_branch {
            let left = scope_counts(|census| census.visit_expr(otherwise));
            self.children.push(left);
        }
    }

    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.note(pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.note_target(&assignment.left);
        syn::visit::visit_expr_assign(self, assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.note_target(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
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
    use super::name_census;

    fn sole(body: &str) -> Vec<String> {
        let block: syn::Block = syn::parse_str(body).expect("the body parses as written");
        name_census(&block).sole.into_iter().collect()
    }

    #[test]
    fn a_name_written_once_is_sole_and_a_shadowed_one_is_not() {
        assert_eq!(sole("{ let value = 1; }"), vec!["value".to_owned()]);
        assert!(sole("{ let value = 1; let value = 2; }").is_empty());
    }

    /// An accumulator is written by its assignment as much as by its
    /// binding. Counting only the binding would state its first value as
    /// the value every later reading sees.
    #[test]
    fn an_assigned_name_is_written_by_the_assignment_too() {
        assert!(sole("{ let mut total = 0; total = 1; }").is_empty());
        assert!(sole("{ let mut total = 0; total += 1; }").is_empty());
    }

    /// A comparison is not an assignment, and its operands are read
    /// rather than written.
    #[test]
    fn a_plain_operator_writes_nothing() {
        assert_eq!(
            sole("{ let total = 0; let _ = total == 1; }"),
            vec!["total".to_owned()]
        );
    }
}
