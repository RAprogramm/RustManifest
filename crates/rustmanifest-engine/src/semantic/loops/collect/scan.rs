// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The scan of a function body: every `for`, `while` and `loop` it
//! contains, and the direct `let` statements of each of their bodies.
//!
//! All three loop forms reach the same analysis, because what matters is
//! the body and the identifiers its header binds, not the keyword. Only
//! statements sitting directly in the loop body are considered; a `let`
//! nested in an inner block belongs to that block's own scan.

use rustmanifest_schema::Range;
use syn::{spanned::Spanned, visit::Visit};

use super::{
    binding::{BoundCollector, pattern_idents},
    candidate::Candidate,
    hoist::hoist_plan,
    mutation::MutatedCollector
};
use crate::semantic::{
    expr::{binding_ident, settled_call},
    loops::{
        origin::{allocation_origin, clone_origin},
        uses::{UseCollector, UseSummary}
    }
};

/// The item a macro written as the whole of an expression reaches, as
/// the compiler settled the path it is written with.
fn macro_path(
    settled: Option<&rustmanifest_schema::Resolution>,
    file: &std::path::Path,
    expression: &syn::Expr
) -> Option<String> {
    let syn::Expr::Macro(invocation) = expression else {
        return None;
    };
    let span = invocation.mac.path.span().byte_range();
    Some(
        settled?
            .path_at(&rustmanifest_schema::Location {
                file:  file.to_path_buf(),
                range: rustmanifest_schema::Range {
                    start: u32::try_from(span.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(span.end).unwrap_or(u32::MAX)
                }
            })?
            .names
            .clone()
    )
}

/// Visitor locating loops and analyzing their direct `let` statements.
pub(super) struct LoopFinder<'out> {
    /// Index of the source under analysis.
    pub(super) source:     usize,
    /// Qualified name of the enclosing function.
    pub(super) function:   String,
    /// Full source text, for line expansion and indentation.
    pub(super) text:       &'out str,
    /// File the visited source is written in, as a resolution keys it.
    pub(super) file:       &'out std::path::Path,
    /// What a resolver settled about the sources, where one ran.
    pub(super) settled:    Option<&'out rustmanifest_schema::Resolution>,
    /// Output collection shared across loops of the function.
    pub(super) candidates: &'out mut Vec<Candidate>
}

impl LoopFinder<'_> {
    /// Analyzes one loop body, given the idents bound by the loop pattern
    /// and the byte offset where the loop expression starts.
    fn analyze_loop(&mut self, pattern: Option<&syn::Pat>, body: &syn::Block, loop_start: usize) {
        let mut bound = Vec::new();
        if let Some(pattern) = pattern {
            pattern_idents(pattern, &mut bound);
        }
        let mut binder = BoundCollector {
            bound: &mut bound
        };
        binder.visit_block(body);
        let mut mutated = MutatedCollector {
            roots:   Vec::new(),
            settled: self.settled,
            file:    self.file
        };
        mutated.visit_block(body);
        for (index, statement) in body.stmts.iter().enumerate() {
            let syn::Stmt::Local(local) = statement else {
                continue;
            };
            let Some(ident) = binding_ident(&local.pat) else {
                continue;
            };
            let Some(init) = &local.init else {
                continue;
            };
            let initialized = settled_call(self.settled, self.file, init.expr.span());
            let reached_macro = macro_path(self.settled, self.file, &init.expr);
            let origin = clone_origin(&init.expr, initialized, &bound, &mutated.roots).or_else(
                || {
                    allocation_origin(
                        &init.expr,
                        initialized,
                        reached_macro.as_deref(),
                        &bound
                    )
                }
            );
            let Some(origin) = origin else {
                continue;
            };
            let name = ident.to_string();
            let mut uses = UseCollector {
                name:    name.clone(),
                summary: UseSummary::default(),
                settled: self.settled,
                file:    self.file
            };
            for later in &body.stmts[index + 1..] {
                uses.visit_stmt(later);
            }
            let bytes = ident.span().byte_range();
            let hoist = hoist_plan(self.text, statement.span().byte_range(), loop_start);
            self.candidates.push(Candidate {
                source: self.source,
                function: self.function.clone(),
                range: Range {
                    start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
                },
                name,
                origin,
                uses: uses.summary,
                hoist
            });
        }
    }
}

impl<'ast> Visit<'ast> for LoopFinder<'_> {
    fn visit_expr_for_loop(&mut self, expr: &'ast syn::ExprForLoop) {
        self.analyze_loop(Some(&expr.pat), &expr.body, expr.span().byte_range().start);
        syn::visit::visit_expr_for_loop(self, expr);
    }

    fn visit_expr_while(&mut self, expr: &'ast syn::ExprWhile) {
        self.analyze_loop(None, &expr.body, expr.span().byte_range().start);
        syn::visit::visit_expr_while(self, expr);
    }

    fn visit_expr_loop(&mut self, expr: &'ast syn::ExprLoop) {
        self.analyze_loop(None, &expr.body, expr.span().byte_range().start);
        syn::visit::visit_expr_loop(self, expr);
    }
}
