// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Candidate collection: walks every function, finds loops, and turns
//! each eligible loop-local `let` into a [`Candidate`] carrying its
//! origin classification and use summary. The rule layer consumes the
//! candidates; this module never decides what is a violation.

use rustmanifest_schema::Range;
use syn::visit::Visit;

use super::{
    origin::{Origin, allocation_origin, clone_origin},
    uses::{UseCollector, UseSummary}
};
use crate::{
    semantic::expr::{
        MUTATING_METHODS, binding_ident, compound_assignment, consuming_method, impl_type_name,
        place_root, token_words
    },
    source::Source
};

/// One loop-local `let` binding eligible for either check.
pub(super) struct Candidate {
    /// Index of the source the binding was parsed from.
    pub(super) source:   usize,
    /// Qualified name of the enclosing function.
    pub(super) function: String,
    /// Byte range of the binding identifier.
    pub(super) range:    Range,
    /// Name of the binding.
    pub(super) name:     String,
    /// How the binding obtained its value.
    pub(super) origin:   Origin,
    /// Classified uses of the binding after its `let`.
    pub(super) uses:     UseSummary
}

/// Collects loop-local `let` candidates from every function in `sources`.
pub(super) fn candidates(sources: &[Source]) -> Vec<Candidate> {
    let mut found = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    let mut finder = LoopFinder {
                        source:     index,
                        function:   function.sig.ident.to_string(),
                        candidates: &mut found
                    };
                    finder.visit_block(&function.block);
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        let mut finder = LoopFinder {
                            source:     index,
                            function:   format!("{type_name}::{}", method.sig.ident),
                            candidates: &mut found
                        };
                        finder.visit_block(&method.block);
                    }
                }
                _ => {}
            }
        }
    }
    found
}

/// Visitor locating loops and analyzing their direct `let` statements.
struct LoopFinder<'out> {
    /// Index of the source under analysis.
    source:     usize,
    /// Qualified name of the enclosing function.
    function:   String,
    /// Output collection shared across loops of the function.
    candidates: &'out mut Vec<Candidate>
}

impl LoopFinder<'_> {
    /// Analyzes one loop body, given the idents bound by the loop pattern.
    fn analyze_loop(&mut self, pattern: Option<&syn::Pat>, body: &syn::Block) {
        let mut bound = Vec::new();
        if let Some(pattern) = pattern {
            pattern_idents(pattern, &mut bound);
        }
        let mut binder = BoundCollector {
            bound: &mut bound
        };
        binder.visit_block(body);
        let mut mutated = MutatedCollector {
            roots: Vec::new()
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
            let origin = clone_origin(&init.expr, &bound, &mutated.roots)
                .or_else(|| allocation_origin(&init.expr, &bound));
            let Some(origin) = origin else {
                continue;
            };
            let name = ident.to_string();
            let mut uses = UseCollector {
                name:    name.clone(),
                summary: UseSummary::default()
            };
            for later in &body.stmts[index + 1..] {
                uses.visit_stmt(later);
            }
            let bytes = ident.span().byte_range();
            self.candidates.push(Candidate {
                source: self.source,
                function: self.function.clone(),
                range: Range {
                    start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
                },
                name,
                origin,
                uses: uses.summary
            });
        }
    }
}

impl<'ast> Visit<'ast> for LoopFinder<'_> {
    fn visit_expr_for_loop(&mut self, expr: &'ast syn::ExprForLoop) {
        self.analyze_loop(Some(&expr.pat), &expr.body);
        syn::visit::visit_expr_for_loop(self, expr);
    }

    fn visit_expr_while(&mut self, expr: &'ast syn::ExprWhile) {
        self.analyze_loop(None, &expr.body);
        syn::visit::visit_expr_while(self, expr);
    }

    fn visit_expr_loop(&mut self, expr: &'ast syn::ExprLoop) {
        self.analyze_loop(None, &expr.body);
        syn::visit::visit_expr_loop(self, expr);
    }
}

/// Visitor collecting every identifier bound by a pattern in a block.
struct BoundCollector<'out> {
    /// Accumulated bound identifiers.
    bound: &'out mut Vec<String>
}

impl<'ast> Visit<'ast> for BoundCollector<'_> {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.bound.push(pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }
}

/// Collects binding idents introduced by a pattern.
fn pattern_idents(pattern: &syn::Pat, out: &mut Vec<String>) {
    let mut collector = BoundCollector {
        bound: out
    };
    collector.visit_pat(pattern);
}

/// Visitor collecting root identifiers that a loop body may mutate.
struct MutatedCollector {
    /// Roots of assignment targets, `&mut` borrows, mutating or consuming
    /// method receivers, and every identifier a macro mentions.
    roots: Vec<String>
}

impl MutatedCollector {
    /// Records the root of a place expression, when it has one.
    fn record(&mut self, expr: &syn::Expr) {
        if let Some(root) = place_root(expr) {
            self.roots.push(root);
        }
    }
}

impl<'ast> Visit<'ast> for MutatedCollector {
    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.record(&assignment.left);
        syn::visit::visit_expr_assign(self, assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.record(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if reference.mutability.is_some() {
            self.record(&reference.expr);
        }
        syn::visit::visit_expr_reference(self, reference);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if MUTATING_METHODS.contains(&method.as_str()) || consuming_method(&method) {
            self.record(&call.receiver);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        self.roots
            .extend(token_words(&invocation.tokens.to_string()));
        syn::visit::visit_macro(self, invocation);
    }
}
