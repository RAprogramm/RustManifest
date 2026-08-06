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

use std::collections::BTreeSet;

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

/// Counts how `block` writes each name.
///
/// Two writings leave no single value behind a name: an assignment,
/// which reaches whatever scope bound the name, and a scope binding one
/// name twice. A name bound again inside a nested scope is neither: the
/// inner binding stands only where it is written, and the reading
/// restores what it shadowed on the way out — so two scopes binding one
/// name are two names, and both are read.
pub(in crate::semantic::behavior) fn name_census(block: &syn::Block) -> NameCensus {
    let mut census = Census {
        bound:        Vec::new(),
        assigned:     BTreeSet::new(),
        double_bound: BTreeSet::new(),
        written:      BTreeSet::new()
    };
    census.enter();
    census.visit_block(block);
    census.leave();
    let sole = census
        .written
        .iter()
        .filter(|name| !census.assigned.contains(*name) && !census.double_bound.contains(*name))
        .cloned()
        .collect();
    NameCensus {
        written: census.written,
        sole
    }
}

/// Walk recording how each name is written.
struct Census {
    /// Names bound in each scope still open, innermost last.
    bound:        Vec<BTreeSet<String>>,
    /// Names an assignment writes anywhere in the body.
    assigned:     BTreeSet<String>,
    /// Names one scope binds more than once.
    double_bound: BTreeSet<String>,
    /// Every name written at all.
    written:      BTreeSet<String>
}

impl Census {
    /// Opens a scope.
    fn enter(&mut self) {
        self.bound.push(BTreeSet::new());
    }

    /// Closes the innermost scope.
    fn leave(&mut self) {
        self.bound.pop();
    }

    /// Records one binding in the innermost open scope.
    fn note_binding(&mut self, name: String) {
        self.written.insert(name.clone());
        if let Some(scope) = self.bound.last_mut()
            && !scope.insert(name.clone())
        {
            self.double_bound.insert(name);
        }
    }

    /// Records one assignment, which reaches whatever scope bound it.
    fn note_assignment(&mut self, target: &syn::Expr) {
        if let syn::Expr::Path(path) = target
            && let Some(name) = path.path.get_ident()
        {
            let name = name.to_string();
            self.written.insert(name.clone());
            self.assigned.insert(name);
        }
    }

    /// Runs `body` inside a scope of its own.
    fn scoped(&mut self, body: impl FnOnce(&mut Self)) {
        self.enter();
        body(self);
        self.leave();
    }
}

impl<'ast> Visit<'ast> for Census {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scoped(|census| syn::visit::visit_block(census, block));
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        self.scoped(|census| syn::visit::visit_arm(census, arm));
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.scoped(|census| syn::visit::visit_expr_closure(census, closure));
    }

    fn visit_expr_for_loop(&mut self, loop_expr: &'ast syn::ExprForLoop) {
        self.scoped(|census| syn::visit::visit_expr_for_loop(census, loop_expr));
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.scoped(|census| {
            census.visit_expr(&conditional.cond);
            syn::visit::visit_block(census, &conditional.then_branch);
        });
        if let Some((_, otherwise)) = &conditional.else_branch {
            self.scoped(|census| census.visit_expr(otherwise));
        }
    }

    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.note_binding(pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.note_assignment(&assignment.left);
        syn::visit::visit_expr_assign(self, assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.note_assignment(&binary.left);
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
