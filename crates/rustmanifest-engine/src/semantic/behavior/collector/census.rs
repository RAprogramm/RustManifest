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
//! Alternatives are counted as alternatives. Two arms of one `match`, or
//! the two halves of an `if`, each bind their own name, and a reading
//! standing inside one of them can see only that one — so sibling
//! branches contribute the largest count any single branch reaches rather
//! than their sum. Adding them called every name bound in two arms
//! twice-written, and refused a value the source settles beyond doubt.
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

/// Counts how often `block` writes each name.
pub(in crate::semantic::behavior) fn name_census(block: &syn::Block) -> NameCensus {
    let mut census = Census {
        counts: BTreeMap::new()
    };
    census.visit_block(block);
    let mut written = BTreeSet::new();
    let mut sole = BTreeSet::new();
    for (name, count) in census.counts {
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

/// Walk counting how many times each name is written.
struct Census {
    /// How often each name is bound or assigned.
    counts: BTreeMap<String, u32>
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

impl Census {
    /// Adds the largest count each name reaches in any one of `branches`.
    ///
    /// The branches are alternatives of each other: at most one of them
    /// runs, and a position inside one never sees the bindings of
    /// another.
    fn note_alternatives(&mut self, branches: Vec<BTreeMap<String, u32>>) {
        let mut largest: BTreeMap<String, u32> = BTreeMap::new();
        for branch in branches {
            for (name, count) in branch {
                let seen = largest.entry(name).or_insert(0);
                *seen = (*seen).max(count);
            }
        }
        for (name, count) in largest {
            *self.counts.entry(name).or_insert(0) += count;
        }
    }
}

/// Counts the names one alternative writes, apart from every other.
fn branch_counts(visit: impl FnOnce(&mut Census)) -> BTreeMap<String, u32> {
    let mut census = Census {
        counts: BTreeMap::new()
    };
    visit(&mut census);
    census.counts
}

impl<'ast> Visit<'ast> for Census {
    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        self.visit_expr(&matched.expr);
        let branches = matched
            .arms
            .iter()
            .map(|arm| branch_counts(|census| census.visit_arm(arm)))
            .collect();
        self.note_alternatives(branches);
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.visit_expr(&conditional.cond);
        let mut branches = vec![branch_counts(|census| {
            census.visit_block(&conditional.then_branch);
        })];
        if let Some((_, otherwise)) = &conditional.else_branch {
            branches.push(branch_counts(|census| census.visit_expr(otherwise)));
        }
        self.note_alternatives(branches);
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
