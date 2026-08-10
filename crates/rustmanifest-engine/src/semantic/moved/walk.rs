// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The body walk of the `use-after-move` check: written order in,
//! proven mentions out.
//!
//! One walker runs per function body. It seeds the provably non-`Copy`
//! owned parameters from the signature, follows statements as written,
//! records a move only in unconditional position, revives a name on
//! rebinding or assignment, and reads macro token streams for bare
//! identifiers and inline template captures — the two spellings a use
//! inside a macro can take.

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::visit::Visit;

use super::MovedUse;
use crate::semantic::{graph::TypeGraph, handover, std_knowledge};

/// Every proven use of a moved-from name in `parsed`.
pub(super) fn moved_uses(parsed: &syn::File, types: &TypeGraph) -> Vec<MovedUse> {
    let mut functions = Functions {
        types,
        foreign: foreign_names(parsed),
        found: Vec::new()
    };
    functions.visit_file(parsed);
    functions.found
}

/// Type names this file imports from outside the workspace tree
/// (`use` paths not rooted in `crate`, `self`, or `super`): a local
/// spelled with such a name may be the import, not the workspace type
/// of the same last segment, so no non-`Copy` claim can rest on it.
fn foreign_names(parsed: &syn::File) -> std::collections::BTreeSet<String> {
    let mut found = std::collections::BTreeSet::new();
    for item in &parsed.items {
        if let syn::Item::Use(used) = item
            && let syn::UseTree::Path(path) = &used.tree
            && !matches!(
                path.ident.to_string().as_str(),
                "crate" | "self" | "super" | "std" | "core" | "alloc"
            )
        {
            leaves(&used.tree, &mut found);
        }
    }
    found
}

/// The leaf names one `use` tree binds.
fn leaves(tree: &syn::UseTree, found: &mut std::collections::BTreeSet<String>) {
    match tree {
        syn::UseTree::Path(path) => leaves(&path.tree, found),
        syn::UseTree::Name(name) => {
            found.insert(name.ident.to_string());
        }
        syn::UseTree::Rename(renamed) => {
            found.insert(renamed.rename.to_string());
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                leaves(item, found);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

/// Finds every function body and runs one walker over each.
struct Functions<'a> {
    types:   &'a TypeGraph,
    foreign: std::collections::BTreeSet<String>,
    found:   Vec<MovedUse>
}

impl<'ast> Visit<'ast> for Functions<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.walk(&item.sig, &item.block);
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.walk(&item.sig, &item.block);
        syn::visit::visit_impl_item_fn(self, item);
    }
}

impl Functions<'_> {
    /// Runs one walker over a signature and its body.
    fn walk(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let mut walker = Walker {
            types:       self.types,
            foreign:     &self.foreign,
            names:       std::collections::BTreeMap::new(),
            conditional: 0,
            found:       Vec::new()
        };
        for input in &sig.inputs {
            if let syn::FnArg::Typed(argument) = input
                && let Some(name) = bound_ident(&argument.pat)
                && let Some(type_name) = owned_type(&argument.ty)
            {
                walker.rebind(&name, Some(type_name));
            }
        }
        walker.visit_block(block);
        self.found.append(&mut walker.found);
    }
}

/// The one name a plain or type-ascribed binding pattern binds.
fn bound_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
        syn::Pat::Type(typed) => bound_ident(&typed.pat),
        _ => None
    }
}

/// The tracked state of one name.
struct Name {
    /// Provably non-`Copy` type of the value it holds.
    type_name: String,
    /// What the value moved into, where it has.
    moved:     Option<String>
}

/// The in-order walk of one body.
struct Walker<'a> {
    types:       &'a TypeGraph,
    foreign:     &'a std::collections::BTreeSet<String>,
    names:       std::collections::BTreeMap<String, Name>,
    conditional: u32,
    found:       Vec<MovedUse>
}

/// Whether the attributes gate the construct by configuration: what
/// runs then depends on the build, which written order cannot prove.
fn cfg_gated(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("cfg"))
}

/// The outer attributes of a statement-position expression, for the
/// variants a statement can carry them on.
fn expr_attrs(expr: &syn::Expr) -> &[syn::Attribute] {
    match expr {
        syn::Expr::Call(held) => &held.attrs,
        syn::Expr::MethodCall(held) => &held.attrs,
        syn::Expr::Macro(held) => &held.attrs,
        syn::Expr::If(held) => &held.attrs,
        syn::Expr::Match(held) => &held.attrs,
        syn::Expr::Block(held) => &held.attrs,
        syn::Expr::ForLoop(held) => &held.attrs,
        syn::Expr::While(held) => &held.attrs,
        syn::Expr::Loop(held) => &held.attrs,
        syn::Expr::Assign(held) => &held.attrs,
        syn::Expr::Unsafe(held) => &held.attrs,
        syn::Expr::Return(held) => &held.attrs,
        _ => &[]
    }
}

impl<'ast> Visit<'ast> for Walker<'_> {
    fn visit_stmt(&mut self, stmt: &'ast syn::Stmt) {
        match stmt {
            syn::Stmt::Item(_) => {}
            syn::Stmt::Local(held) if cfg_gated(&held.attrs) => {}
            syn::Stmt::Expr(held, _) if cfg_gated(expr_attrs(held)) => {}
            syn::Stmt::Macro(held) if cfg_gated(&held.attrs) => {}
            syn::Stmt::Macro(held) => self.macro_scan(&held.mac),
            _ => syn::visit::visit_stmt(self, stmt)
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let bound = bound_ident(&local.pat);
        let mut recognized = None;
        if let Some(init) = &local.init {
            if let Some(ident) = bare(&init.expr) {
                let into = bound.as_deref().map_or_else(
                    || "a `let` binding".to_owned(),
                    |name| format!("let {name}")
                );
                self.handed(&ident, &into, syn::spanned::Spanned::span(&init.expr));
            } else {
                recognized = recognized_type(&init.expr);
                self.visit_expr(&init.expr);
            }
        }
        if let Some(name) = bound {
            self.rebind(&name, recognized);
        }
    }

    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        match expr {
            syn::Expr::If(_)
            | syn::Expr::Match(_)
            | syn::Expr::While(_)
            | syn::Expr::ForLoop(_)
            | syn::Expr::Loop(_) => {
                self.conditional += 1;
                syn::visit::visit_expr(self, expr);
                self.conditional -= 1;
            }
            syn::Expr::Closure(closure) => {
                let mut shadowed = Vec::new();
                for input in &closure.inputs {
                    if let Some(name) = bound_ident(input)
                        && let Some(state) = self.names.remove(&name)
                    {
                        shadowed.push((name, state));
                    }
                }
                self.conditional += 1;
                syn::visit::visit_expr(self, expr);
                self.conditional -= 1;
                for (name, state) in shadowed {
                    self.names.insert(name, state);
                }
            }
            syn::Expr::Call(call) => {
                let into = call_label(&call.func);
                for argument in &call.args {
                    self.argument(argument, &into);
                }
            }
            syn::Expr::MethodCall(called) => {
                let method = called.method.to_string();
                if let Some(ident) = bare(&called.receiver) {
                    if self.is_moved(&ident) {
                        self.mention(&ident, syn::spanned::Spanned::span(&called.receiver));
                    } else if std_knowledge::consuming_method(&method) {
                        self.moved(&ident, &format!("{method}()"));
                    }
                } else {
                    self.visit_expr(&called.receiver);
                }
                for argument in &called.args {
                    self.argument(argument, &format!("{method}()"));
                }
            }
            syn::Expr::Assign(assign) => {
                self.visit_expr(&assign.right);
                if let Some(ident) = bare(&assign.left) {
                    if let Some(name) = self.names.get_mut(&ident) {
                        name.moved = None;
                    }
                } else {
                    self.visit_expr(&assign.left);
                }
            }
            syn::Expr::Path(path) => {
                if let Some(ident) = single(path) {
                    self.mention(&ident, syn::spanned::Spanned::span(path));
                }
            }
            syn::Expr::Macro(held) => self.macro_scan(&held.mac),
            _ => syn::visit::visit_expr(self, expr)
        }
    }
}

impl Walker<'_> {
    /// Tracks or forgets `name`: tracked only when its type is provably
    /// not `Copy` and its name is not shadowed by a foreign import,
    /// forgotten otherwise so no later claim rests on it.
    fn rebind(&mut self, name: &str, type_name: Option<String>) {
        match type_name
            .filter(|held| !self.foreign.contains(held))
            .filter(|held| handover::copyness(self.types, held) == Some(false))
        {
            Some(held) => {
                self.names.insert(
                    name.to_owned(),
                    Name {
                        type_name: held,
                        moved:     None
                    }
                );
            }
            None => {
                self.names.remove(name);
            }
        }
    }

    /// One written argument: a bare tracked name moves or is flagged,
    /// anything else is walked for the uses inside it.
    fn argument(&mut self, expr: &syn::Expr, into: &str) {
        if let Some(ident) = bare(expr) {
            self.handed(&ident, into, syn::spanned::Spanned::span(expr));
        } else {
            self.visit_expr(expr);
        }
    }

    /// A bare name handed over: a mention where already moved, a move
    /// in unconditional position, nothing otherwise.
    fn handed(&mut self, ident: &str, into: &str, span: Span) {
        if self.is_moved(ident) {
            self.mention(ident, span);
        } else {
            self.moved(ident, into);
        }
    }

    /// Records the move of `ident` where the position is unconditional.
    fn moved(&mut self, ident: &str, into: &str) {
        if self.conditional == 0
            && let Some(name) = self.names.get_mut(ident)
            && name.moved.is_none()
        {
            name.moved = Some(into.to_owned());
        }
    }

    /// Whether `ident` is tracked and already moved from.
    fn is_moved(&self, ident: &str) -> bool {
        self.names
            .get(ident)
            .is_some_and(|name| name.moved.is_some())
    }

    /// Flags one mention of `ident` at `span`, where it is moved from.
    fn mention(&mut self, ident: &str, span: Span) {
        if let Some(name) = self.names.get(ident)
            && let Some(into) = &name.moved
        {
            self.found.push(MovedUse {
                name:      ident.to_owned(),
                type_name: name.type_name.clone(),
                into:      into.clone(),
                range:     ranged(span)
            });
        }
    }

    /// Reads a macro's tokens: a bare identifier and an inline template
    /// capture both count as the use they spell.
    fn macro_scan(&mut self, invoked: &syn::Macro) {
        self.token_scan(invoked.tokens.clone());
    }

    /// One token tree level: idents are mentions, string literals are
    /// searched for inline captures of moved names.
    fn token_scan(&mut self, stream: TokenStream) {
        for token in stream {
            match token {
                TokenTree::Group(group) => self.token_scan(group.stream()),
                TokenTree::Ident(ident) => self.mention(&ident.to_string(), ident.span()),
                TokenTree::Literal(literal) => {
                    let written = literal.to_string();
                    let captured: Vec<String> = self
                        .names
                        .iter()
                        .filter(|(_, name)| name.moved.is_some())
                        .filter(|(name, _)| {
                            written.contains(&format!("{{{name}}}"))
                                || written.contains(&format!("{{{name}:"))
                        })
                        .map(|(name, _)| name.clone())
                        .collect();
                    for name in captured {
                        self.mention(&name, literal.span());
                    }
                }
                TokenTree::Punct(_) => {}
            }
        }
    }
}

/// The one identifier a bare single-segment path expression spells.
fn bare(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Path(path) = expr {
        single(path)
    } else {
        None
    }
}

/// The one identifier of a single-segment path without qualifiers.
fn single(path: &syn::ExprPath) -> Option<String> {
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    if !segment.arguments.is_empty() {
        return None;
    }
    Some(segment.ident.to_string())
}

/// The callee of a written call, as written.
fn call_label(func: &syn::Expr) -> String {
    if let syn::Expr::Path(path) = func {
        let segments: Vec<String> = path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        format!("{}()", segments.join("::"))
    } else {
        "the call".to_owned()
    }
}

/// The declared type of an owned parameter, where the declaration
/// names one plainly.
fn owned_type(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(path) = ty {
        Some(path.path.segments.last()?.ident.to_string())
    } else {
        None
    }
}

/// The type a recognizable initializer provably produces.
fn recognized_type(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Call(call) => {
            if let syn::Expr::Path(path) = call.func.as_ref() {
                let segments = &path.path.segments;
                if segments.len() >= 2 {
                    return segments
                        .iter()
                        .rev()
                        .nth(1)
                        .map(|segment| segment.ident.to_string());
                }
            }
            None
        }
        syn::Expr::Struct(built) => built
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Macro(held) => {
            match held.mac.path.segments.last()?.ident.to_string().as_str() {
                "format" => Some("String".to_owned()),
                "vec" => Some("Vec".to_owned()),
                _ => None
            }
        }
        syn::Expr::MethodCall(called) if called.method == "to_string" => Some("String".to_owned()),
        _ => None
    }
}

/// The schema range of one span.
fn ranged(span: Span) -> rustmanifest_schema::Range {
    let bytes = span.byte_range();
    rustmanifest_schema::Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}
