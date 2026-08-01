// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Loop-body ownership checks: `loop-invariant-clone` and
//! `loop-allocation`.
//!
//! `loop-invariant-clone` reports a `let` directly inside a loop body that
//! clones a place whose root is neither bound nor mutated inside the loop,
//! when the copy itself is only ever read — one clone before the loop (or a
//! borrow) would do.
//!
//! `loop-allocation` reports a `let` directly inside a loop body that
//! allocates a fresh container (`Vec::new`, `String::new`,
//! `with_capacity`, empty `vec![]`) which is used during the iteration but
//! never leaves it — hoisting the buffer and clearing it per iteration
//! avoids one heap allocation per pass.
//!
//! Both checks are conservative on the same fronts: only `let` statements
//! that are direct children of the loop block are considered; any
//! unclassified bare use of the binding (a move, a return, a call
//! argument) disqualifies it; any mention of the binding inside a macro
//! invocation disqualifies it, because macros can move or mutate what they
//! mention; and a clone receiver mentioned in any macro or mutated
//! anywhere in the loop is treated as unstable.

use rustmanifest_schema::{Range, TraceStep};
use syn::visit::Visit;

use super::{behavior::MUTATING_METHODS, insight::Insight};
use crate::source::Source;

/// Reports loop-invariant clones that could be hoisted out of the loop.
pub(super) fn clone_insights(sources: &[Source]) -> Vec<Insight> {
    let mut found = Vec::new();
    for candidate in candidates(sources) {
        let Origin::Clone {
            ref chain,
            ref method,
            stable
        } = candidate.origin
        else {
            continue;
        };
        let uses = &candidate.uses;
        if !stable
            || uses.moves > 0
            || uses.mutations > 0
            || uses.mut_borrows > 0
            || uses.calls > 0
            || uses.bare > 0
            || uses.macro_mentions > 0
            || uses.field_reads + uses.shared_borrows == 0
        {
            continue;
        }
        let name = &candidate.name;
        let detail = format!(
            "fn `{}` clones `{chain}` into `{name}` on every loop iteration, but `{name}` is \
             only read and `{chain}` never changes inside the loop; clone once before the loop \
             or borrow instead",
            candidate.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!("`{name}` is initialized from `{chain}.{method}()` inside a loop"),
                reason: "the `let` sits directly in a `for`/`while`/`loop` block".to_owned()
            },
            TraceStep {
                fact:   format!("`{chain}` is neither rebound nor mutated inside the loop"),
                reason: "no assignment, `&mut` borrow, mutating call, or macro targets its root"
                    .to_owned()
            },
            TraceStep {
                fact:   format!("every observed use of `{name}` is a read"),
                reason: "only field reads and shared borrows were seen after the `let`".to_owned()
            },
        ];
        found.push(Insight::new(
            candidate.source,
            candidate.range,
            detail,
            trace
        ));
    }
    found
}

/// Reports per-iteration container allocations that never escape the
/// iteration.
pub(super) fn allocation_insights(sources: &[Source]) -> Vec<Insight> {
    let mut found = Vec::new();
    for candidate in candidates(sources) {
        let Origin::Allocation {
            ref call
        } = candidate.origin
        else {
            continue;
        };
        let uses = &candidate.uses;
        let reads_and_writes = uses.field_reads
            + uses.shared_borrows
            + uses.mut_borrows
            + uses.mutations
            + uses.calls;
        if uses.moves > 0 || uses.bare > 0 || uses.macro_mentions > 0 || reads_and_writes == 0 {
            continue;
        }
        let name = &candidate.name;
        let detail = format!(
            "fn `{}` allocates `{name}` with `{call}` on every loop iteration and the value \
             never leaves the iteration; hoist the buffer out of the loop and `.clear()` it \
             instead",
            candidate.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!("`{name}` is initialized from `{call}` inside a loop"),
                reason: "the `let` sits directly in a `for`/`while`/`loop` block".to_owned()
            },
            TraceStep {
                fact:   format!("`{name}` never escapes the iteration"),
                reason: "it is used, but never moved, returned, or passed to a macro".to_owned()
            },
        ];
        found.push(Insight::new(
            candidate.source,
            candidate.range,
            detail,
            trace
        ));
    }
    found
}

/// How a loop-local `let` binding obtained its value.
enum Origin {
    /// `<place>.clone()` / `.to_owned()` / `.to_string()`.
    Clone {
        /// Rendered place expression of the receiver, e.g. `self.config`.
        chain:  String,
        /// The cloning method that was called.
        method: String,
        /// Whether the receiver's root is neither bound nor mutated inside
        /// the loop.
        stable: bool
    },
    /// A fresh container allocation.
    Allocation {
        /// Rendered allocation call, e.g. `Vec::with_capacity`.
        call: String
    }
}

/// One loop-local `let` binding eligible for either check.
struct Candidate {
    /// Index of the source the binding was parsed from.
    source:   usize,
    /// Qualified name of the enclosing function.
    function: String,
    /// Byte range of the binding identifier.
    range:    Range,
    /// Name of the binding.
    name:     String,
    /// How the binding obtained its value.
    origin:   Origin,
    /// Classified uses of the binding after its `let`.
    uses:     UseSummary
}

/// Classified uses of one binding within the rest of a loop body.
#[derive(Default)]
struct UseSummary {
    /// Reads of a field of the binding.
    field_reads:    u32,
    /// Shared borrows of the binding or one of its fields.
    shared_borrows: u32,
    /// Mutable borrows of the binding or one of its fields.
    mut_borrows:    u32,
    /// Assignments to the binding and known mutating method calls.
    mutations:      u32,
    /// Method calls that are neither known-mutating nor consuming.
    calls:          u32,
    /// Consuming method calls (`into_*`, `leak`).
    moves:          u32,
    /// Bare uses that could move the value (call argument, return, push).
    bare:           u32,
    /// Mentions inside macro invocations, which hide the use shape.
    macro_mentions: u32
}

/// Collects loop-local `let` candidates from every function in `sources`.
fn candidates(sources: &[Source]) -> Vec<Candidate> {
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

/// Renders the last path segment of an impl's self type.
fn impl_type_name(block: &syn::ItemImpl) -> Option<String> {
    let syn::Type::Path(path) = block.self_ty.as_ref() else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
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

/// Classifies `expr` as a clone of a stable or unstable place.
fn clone_origin(expr: &syn::Expr, bound: &[String], mutated: &[String]) -> Option<Origin> {
    let syn::Expr::MethodCall(call) = expr else {
        return None;
    };
    let method = call.method.to_string();
    if !matches!(method.as_str(), "clone" | "to_owned" | "to_string") || !call.args.is_empty() {
        return None;
    }
    let chain = place_chain(&call.receiver)?;
    let root = chain.split('.').next().unwrap_or(&chain).to_owned();
    let stable = !bound.contains(&root) && !mutated.contains(&root);
    Some(Origin::Clone {
        chain,
        method,
        stable
    })
}

/// Container constructors recognized as fresh heap allocations.
const ALLOCATING_TYPES: [&str; 2] = ["String", "Vec"];

/// Classifies `expr` as a fresh container allocation whose arguments do
/// not depend on loop-bound values.
fn allocation_origin(expr: &syn::Expr, bound: &[String]) -> Option<Origin> {
    match expr {
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let segments = &path.path.segments;
            if segments.len() != 2 {
                return None;
            }
            let owner = segments[0].ident.to_string();
            let constructor = segments[1].ident.to_string();
            if !ALLOCATING_TYPES.contains(&owner.as_str())
                || !matches!(constructor.as_str(), "new" | "with_capacity")
                || !call.args.iter().all(|arg| invariant_argument(arg, bound))
            {
                return None;
            }
            Some(Origin::Allocation {
                call: format!("{owner}::{constructor}")
            })
        }
        syn::Expr::Macro(invocation)
            if invocation.mac.path.is_ident("vec") && invocation.mac.tokens.is_empty() =>
        {
            Some(Origin::Allocation {
                call: "vec![]".to_owned()
            })
        }
        _ => None
    }
}

/// Returns whether a constructor argument is a literal or an identifier
/// that is not bound inside the loop, so hoisting keeps its value.
fn invariant_argument(argument: &syn::Expr, bound: &[String]) -> bool {
    match argument {
        syn::Expr::Lit(_) => true,
        syn::Expr::Path(path) => path
            .path
            .get_ident()
            .is_some_and(|ident| !bound.contains(&ident.to_string())),
        _ => false
    }
}

/// Extracts the identifier a `let` pattern binds, `mut` or not.
fn binding_ident(pattern: &syn::Pat) -> Option<&syn::Ident> {
    match pattern {
        syn::Pat::Ident(ident) => Some(&ident.ident),
        syn::Pat::Type(typed) => binding_ident(&typed.pat),
        _ => None
    }
}

/// Renders a place expression (`root`, `root.field`, `self.field.inner`)
/// or returns `None` for anything that is not a plain place.
fn place_chain(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => {
            let base = place_chain(&field.base)?;
            let member = match &field.member {
                syn::Member::Named(ident) => ident.to_string(),
                syn::Member::Unnamed(index) => index.index.to_string()
            };
            Some(format!("{base}.{member}"))
        }
        _ => None
    }
}

/// Returns the root identifier of a place expression, seeing through
/// field access, indexing, parentheses, and dereference.
fn place_root(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => place_root(&field.base),
        syn::Expr::Index(index) => place_root(&index.expr),
        syn::Expr::Paren(paren) => place_root(&paren.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            place_root(&unary.expr)
        }
        syn::Expr::MethodCall(call) => place_root(&call.receiver),
        _ => None
    }
}

/// Returns whether `method` consumes its receiver.
fn consuming_method(method: &str) -> bool {
    method.starts_with("into") || method == "leak"
}

/// Splits macro token text into identifier-shaped words.
fn token_words(tokens: &str) -> Vec<String> {
    tokens
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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

/// Returns whether `op` is a compound assignment operator.
const fn compound_assignment(op: syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_)
            | syn::BinOp::BitXorAssign(_)
            | syn::BinOp::BitAndAssign(_)
            | syn::BinOp::BitOrAssign(_)
            | syn::BinOp::ShlAssign(_)
            | syn::BinOp::ShrAssign(_)
    )
}

/// Visitor classifying every use of one binding.
struct UseCollector {
    /// The binding under observation.
    name:    String,
    /// Classified counts.
    summary: UseSummary
}

impl UseCollector {
    /// Returns whether `expr` is a bare path naming the binding.
    fn is_binding(&self, expr: &syn::Expr) -> bool {
        matches!(expr, syn::Expr::Path(path) if path.path.is_ident(self.name.as_str()))
    }

    /// Returns whether the root of a place expression is the binding.
    fn roots_in_binding(&self, expr: &syn::Expr) -> bool {
        place_root(expr).is_some_and(|root| root == self.name)
    }
}

impl<'ast> Visit<'ast> for UseCollector {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.is_binding(&call.receiver) {
            let method = call.method.to_string();
            if consuming_method(&method) {
                self.summary.moves += 1;
            } else if MUTATING_METHODS.contains(&method.as_str()) {
                self.summary.mutations += 1;
            } else {
                self.summary.calls += 1;
            }
            for argument in &call.args {
                self.visit_expr(argument);
            }
            return;
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if self.is_binding(&field.base) {
            self.summary.field_reads += 1;
            return;
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if self.roots_in_binding(&reference.expr) {
            if reference.mutability.is_some() {
                self.summary.mut_borrows += 1;
            } else {
                self.summary.shared_borrows += 1;
            }
            return;
        }
        syn::visit::visit_expr_reference(self, reference);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        if self.roots_in_binding(&assignment.left) {
            self.summary.mutations += 1;
            self.visit_expr(&assignment.right);
            return;
        }
        syn::visit::visit_expr_assign(self, assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) && self.roots_in_binding(&binary.left) {
            self.summary.mutations += 1;
            self.visit_expr(&binary.right);
            return;
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.path.is_ident(self.name.as_str()) {
            self.summary.bare += 1;
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if token_words(&invocation.tokens.to_string()).contains(&self.name) {
            self.summary.macro_mentions += 1;
        }
        syn::visit::visit_macro(self, invocation);
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
    use std::path::PathBuf;

    use super::{allocation_insights, clone_insights};
    use crate::source::Source;

    fn sources(text: &str) -> [Source; 1] {
        [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn invariant_clone_read_only_in_loop_is_reported() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn total(config: Config, jobs: &[u32]) -> u32 {\n    \
             let mut total = 0;\n    for job in jobs {\n        let snapshot = config.clone();\n   \
             total += snapshot.retries + job;\n    }\n    total\n}"
        );
        let insights = clone_insights(&sources);
        assert_eq!(insights.len(), 1);
        assert!(insights[0].detail.contains("`config`"));
        assert_eq!(insights[0].trace.len(), 3);
    }

    #[test]
    fn clone_of_the_loop_variable_passes() {
        let sources = sources(
            "fn copies(items: &[String]) -> u32 {\n    let mut seen = 0;\n    for item in items \
             {\n        let copy = item.clone();\n        seen += copy.len() as u32;\n    }\n    \
             seen\n}"
        );
        assert!(clone_insights(&sources).is_empty());
    }

    #[test]
    fn mutated_clone_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn bump(config: Config, jobs: &[u32]) -> u32 {\n    \
             let mut last = 0;\n    for _job in jobs {\n        let mut snapshot = \
             config.clone();\n        snapshot.retries += 1;\n        last = snapshot.retries;\n  \
             }\n    last\n}"
        );
        assert!(clone_insights(&sources).is_empty());
    }

    #[test]
    fn moved_clone_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn keep(config: Config, jobs: &[u32]) -> \
             Vec<Config> {\n    let mut kept = Vec::new();\n    for _job in jobs {\n        let \
             snapshot = config.clone();\n        kept.push(snapshot);\n    }\n    kept\n}"
        );
        assert!(clone_insights(&sources).is_empty());
    }

    #[test]
    fn clone_of_a_receiver_mutated_in_the_loop_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn churn(mut config: Config, jobs: &[u32]) -> u32 \
             {\n    let mut total = 0;\n    for _job in jobs {\n        let snapshot = \
             config.clone();\n        total += snapshot.retries;\n        config.retries += 1;\n  \
             }\n    total\n}"
        );
        assert!(clone_insights(&sources).is_empty());
    }

    #[test]
    fn clone_outside_a_loop_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn once(config: Config) -> u32 {\n    let snapshot \
             = config.clone();\n    snapshot.retries\n}"
        );
        assert!(clone_insights(&sources).is_empty());
    }

    #[test]
    fn clone_used_in_a_macro_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn shout(config: Config, jobs: &[u32]) {\n    for \
             _job in jobs {\n        let snapshot = config.clone();\n        \
             println!(\"{}\", snapshot.retries);\n    }\n}"
        );
        assert!(clone_insights(&sources).is_empty());
    }

    #[test]
    fn invariant_clone_in_a_while_loop_is_reported() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn wait(config: Config) -> u32 {\n    let mut left \
             = 3;\n    while left > 0 {\n        let snapshot = config.clone();\n        left -= \
             snapshot.retries;\n    }\n    left\n}"
        );
        assert_eq!(clone_insights(&sources).len(), 1);
    }

    #[test]
    fn fresh_buffer_used_but_never_escaping_is_reported() {
        let sources = sources(
            "fn widths(values: &[u32]) -> usize {\n    let mut width = 0;\n    for _value in \
             values {\n        let mut line = String::with_capacity(16);\n        \
             line.push('#');\n        width += line.len();\n    }\n    width\n}"
        );
        let insights = allocation_insights(&sources);
        assert_eq!(insights.len(), 1);
        assert!(insights[0].detail.contains("String::with_capacity"));
        assert_eq!(insights[0].trace.len(), 2);
    }

    #[test]
    fn buffer_moved_out_of_the_iteration_passes() {
        let sources = sources(
            "fn lines(values: &[u32]) -> Vec<String> {\n    let mut lines = Vec::new();\n    for \
             _value in values {\n        let mut line = String::new();\n        \
             line.push('#');\n        lines.push(line);\n    }\n    lines\n}"
        );
        assert!(allocation_insights(&sources).is_empty());
    }

    #[test]
    fn empty_vec_macro_in_loop_is_reported() {
        let sources = sources(
            "fn sums(values: &[u32]) -> u32 {\n    let mut total = 0;\n    for value in values \
             {\n        let mut batch = vec![];\n        batch.push(value);\n        total += \
             batch.len() as u32;\n    }\n    total\n}"
        );
        assert_eq!(allocation_insights(&sources).len(), 1);
    }

    #[test]
    fn unused_buffer_passes() {
        let sources = sources(
            "fn idle(values: &[u32]) {\n    for _value in values {\n        let _scratch: \
             Vec<u32> = Vec::new();\n    }\n}"
        );
        assert!(allocation_insights(&sources).is_empty());
    }

    #[test]
    fn capacity_depending_on_the_loop_variable_passes() {
        let sources = sources(
            "fn sized(values: &[usize]) -> usize {\n    let mut width = 0;\n    for value in \
             values {\n        let line = String::with_capacity(*value);\n        width += \
             line.len();\n    }\n    width\n}"
        );
        assert!(allocation_insights(&sources).is_empty());
    }

    #[test]
    fn buffer_consumed_by_an_into_method_passes() {
        let sources = sources(
            "fn bytes(values: &[u32]) -> usize {\n    let mut total = 0;\n    for _value in \
             values {\n        let line = String::new();\n        total += \
             line.into_bytes().len();\n    }\n    total\n}"
        );
        assert!(allocation_insights(&sources).is_empty());
    }
}
