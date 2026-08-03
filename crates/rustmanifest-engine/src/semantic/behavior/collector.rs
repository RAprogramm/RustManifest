// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Body analysis: the syntax visitor that accumulates behavioral facts —
//! reads, writes, constructions, effects, clones, and allocations — from
//! one function body against its signature-derived parameter types.

use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;

use super::{
    Returned, Scoped,
    signature::{element_type, type_last_segment},
    types::{Effect, FunctionFacts}
};
use crate::semantic::{
    expr::{MUTATING_METHODS, compound_assignment},
    knowledge,
    std_knowledge::EXTERNAL_CONSTRUCTORS
};

/// Print-family macros with the stream each one writes to and whether it
/// appends a line terminator. Every entry also carries
/// [`Effect::ConsoleIo`], which stays the coarse summary category.
const OUTPUT_MACROS: [(&str, super::output::Stream, super::output::Newline); 5] = [
    (
        "dbg",
        super::output::Stream::Stderr,
        super::output::Newline::Appended
    ),
    (
        "eprint",
        super::output::Stream::Stderr,
        super::output::Newline::None
    ),
    (
        "eprintln",
        super::output::Stream::Stderr,
        super::output::Newline::Appended
    ),
    (
        "print",
        super::output::Stream::Stdout,
        super::output::Newline::None
    ),
    (
        "println",
        super::output::Stream::Stdout,
        super::output::Newline::Appended
    )
];

/// Iterator and `Option` adapters whose single-parameter closure
/// receives one element of the adapted collection or option.
const ITERATOR_ADAPTERS: [&str; 18] = [
    "all",
    "and_then",
    "any",
    "filter",
    "filter_map",
    "find",
    "find_map",
    "flat_map",
    "for_each",
    "inspect",
    "is_some_and",
    "map",
    "map_or",
    "map_or_else",
    "position",
    "retain",
    "skip_while",
    "take_while"
];

/// Iterator constructors and element accessors that keep the
/// collection's element type.
const ITERATOR_SOURCES: [&str; 7] = [
    "drain",
    "first",
    "get",
    "into_iter",
    "iter",
    "iter_mut",
    "last"
];

/// Macros that end the current path rather than returning to it.
const DIVERGING_MACROS: [&str; 4] = ["panic", "todo", "unimplemented", "unreachable"];

/// Returns whether a statement is gated by a configuration attribute
/// this tier does not resolve.
fn gated(statement: &syn::Stmt) -> bool {
    let attributes: &[syn::Attribute] = match statement {
        syn::Stmt::Local(local) => &local.attrs,
        syn::Stmt::Expr(syn::Expr::Block(block), _) => &block.attrs,
        syn::Stmt::Macro(invocation) => &invocation.attrs,
        syn::Stmt::Item(syn::Item::Fn(function)) => &function.attrs,
        _ => &[]
    };
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg"))
}

/// Returns whether a statement unconditionally ends the current path, so
/// everything after it in the same block never runs.
fn diverges(statement: &syn::Stmt) -> bool {
    match statement {
        syn::Stmt::Expr(expression, _) => match expression {
            syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_) => true,
            syn::Expr::Call(call) => process_exit(&call.func),
            syn::Expr::Macro(invocation) => diverging_macro(&invocation.mac),
            _ => false
        },
        syn::Stmt::Macro(invocation) => diverging_macro(&invocation.mac),
        _ => false
    }
}

/// Returns whether a call expression names the process-ending pair.
fn process_exit(callee: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = callee else {
        return false;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    matches!(segments.last().map(String::as_str), Some("exit" | "abort"))
        && segments.iter().any(|segment| segment == "process")
}

/// Returns whether a macro invocation ends the current path.
fn diverging_macro(invocation: &syn::Macro) -> bool {
    invocation
        .path
        .segments
        .last()
        .is_some_and(|segment| DIVERGING_MACROS.contains(&segment.ident.to_string().as_str()))
}

/// Returns whether a statement may leave the function before the next
/// one runs: a `?` operator or a conditional `return` inside it.
fn exits(statement: &syn::Stmt) -> bool {
    struct Scan {
        found: bool
    }
    impl<'ast> Visit<'ast> for Scan {
        fn visit_expr_try(&mut self, tried: &'ast syn::ExprTry) {
            self.found = true;
            syn::visit::visit_expr_try(self, tried);
        }

        fn visit_expr_return(&mut self, returned: &'ast syn::ExprReturn) {
            self.found = true;
            syn::visit::visit_expr_return(self, returned);
        }
    }
    let mut scan = Scan {
        found: false
    };
    scan.visit_stmt(statement);
    scan.found
}

/// Syntax visitor accumulating facts from one function body.
pub(super) struct FactCollector {
    /// Facts collected so far.
    pub(super) facts:       FunctionFacts,
    /// Parameter name to type-name mapping from the signature.
    pub(super) param_types: BTreeMap<String, String>,
    /// Binding name to collection element type, from the signature.
    pub(super) elem_types:  BTreeMap<String, String>,
    /// Workspace functions to their return types, scoped by source.
    pub(super) fn_returns:  Scoped<Returned>,
    /// `Type.field` keys to field type names, scoped by source.
    pub(super) field_types: Scoped<String>,
    /// `Type.field` keys of collection fields to element types.
    pub(super) field_elems: Scoped<String>,
    /// `Type.field` keys of wrapper fields to their wrapped inner type.
    pub(super) field_wraps: Scoped<String>,
    /// Crate identity of every source, for crate-proximity resolution.
    pub(super) crates:      Vec<String>,
    /// Source index of the function under analysis.
    pub(super) src:         usize,
    /// Enclosing impl block's generic parameters to their first trait
    /// bound, so a generic-typed field resolves as its bound.
    pub(super) bounds:      BTreeMap<String, String>,
    /// Let bindings holding a wrapper value with a known wrapped inner
    /// type, cloned or borrowed off a wrapper field.
    pub(super) local_wraps: BTreeMap<String, String>,
    /// Imported names of the enclosing file to their full `use` paths.
    pub(super) imports:     BTreeMap<String, String>,
    /// Let bindings holding a value provably constructed in this body.
    pub(super) fresh:       BTreeSet<String>,
    /// Constructs currently enclosing the visited node, outermost first.
    pub(super) guards:      Vec<super::output::Guard>,
    /// Exits that may leave the function before the visited node.
    pub(super) prior_exits: u32,
    /// Whether the visited node follows an unconditional divergence.
    pub(super) dead:        bool
}

impl FactCollector {
    /// Resolves a return-table key for this function's scope.
    fn ret(&self, key: &str) -> Option<&Returned> {
        self.fn_returns.resolve(key, self.src, &self.crates)
    }

    /// Resolves a field-type key for this function's scope.
    fn field(&self, key: &str) -> Option<&String> {
        self.field_types.resolve(key, self.src, &self.crates)
    }

    /// Resolves a field element-type key for this function's scope.
    fn elem(&self, key: &str) -> Option<&String> {
        self.field_elems.resolve(key, self.src, &self.crates)
    }

    /// Resolves a wrapper inner-type key for this function's scope.
    fn wrap(&self, key: &str) -> Option<&String> {
        self.field_wraps.resolve(key, self.src, &self.crates)
    }

    /// Renders a field expression as `Owner.field` when the base is a
    /// typed parameter or `self`, or `?.field` otherwise.
    fn field_key(&self, field: &syn::ExprField) -> Option<String> {
        let syn::Member::Named(name) = &field.member else {
            return None;
        };
        let owner = match field.base.as_ref() {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map_or("?", String::as_str),
            _ => "?"
        };
        Some(format!("{owner}.{name}"))
    }

    /// Records the outermost field of an assignment target as a write —
    /// or as a build when the base is a freshly constructed local — and
    /// counts the base parameter's use so a written-through parameter
    /// never looks read-only.
    fn record_write_target(&mut self, target: &syn::Expr) {
        if let syn::Expr::Field(field) = target {
            if let Some(key) = self.field_key(field) {
                self.record_field_write(field, key);
            }
            if let syn::Expr::Path(path) = field.base.as_ref()
                && let Some(ident) = path.path.get_ident()
            {
                self.bump_param(&ident.to_string(), false);
            }
        }
    }

    /// Routes one field write to `builds` when its base binding holds a
    /// value constructed in this body, to `writes` otherwise.
    fn record_field_write(&mut self, field: &syn::ExprField, key: String) {
        let fresh_base = matches!(
            field.base.as_ref(),
            syn::Expr::Path(path) if path
                .path
                .get_ident()
                .is_some_and(|ident| self.fresh.contains(&ident.to_string()))
        );
        if fresh_base {
            self.facts.builds.push(key);
        } else {
            self.facts.writes.push(key);
        }
    }

    /// Counts one use of the parameter named `name`; `field_read` marks a
    /// use that is only a field read.
    fn bump_param(&mut self, name: &str, field_read: bool) {
        if let Some(param) = self
            .facts
            .params
            .iter_mut()
            .find(|param| param.name == name)
        {
            param.uses += 1;
            if field_read {
                param.field_reads += 1;
            }
        }
    }

    /// Element type of an iterable expression: a binding or field of a
    /// known collection type, seen through references, parentheses, and
    /// the plain iterator constructors.
    fn element_of(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Reference(reference) => self.element_of(&reference.expr),
            syn::Expr::Paren(inner) => self.element_of(&inner.expr),
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.elem_types.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => self
                .field_key(field)
                .and_then(|key| self.elem(&key))
                .cloned(),
            syn::Expr::MethodCall(call)
                if ITERATOR_SOURCES.contains(&call.method.to_string().as_str()) =>
            {
                self.element_of(&call.receiver)
            }
            _ => None
        }
    }

    /// Element type behind an `Option`-returning collection accessor —
    /// `get`, `first`, `last` — on a known element type.
    fn optional_element(&self, expression: &syn::Expr) -> Option<String> {
        let syn::Expr::MethodCall(call) = expression else {
            return None;
        };
        if !matches!(call.method.to_string().as_str(), "get" | "first" | "last") {
            return None;
        }
        self.element_of(&call.receiver)
    }

    /// Binds one loop or closure pattern against the iterated element
    /// type, seeing through `enumerate`'s index tuple.
    fn bind_element(&mut self, pattern: &syn::Pat, iterable: &syn::Expr) {
        if let syn::Expr::MethodCall(call) = iterable
            && call.method == "enumerate"
        {
            if let syn::Pat::Tuple(tuple) = pattern
                && tuple.elems.len() == 2
                && let (Some(name), Some(element)) = (
                    binding_name(&tuple.elems[1]),
                    self.element_of(&call.receiver)
                )
            {
                self.param_types.insert(name, element);
            }
            return;
        }
        if let (Some(name), Some(element)) = (binding_name(pattern), self.element_of(iterable)) {
            self.param_types.insert(name, element);
        }
    }

    /// Wrapped inner type of a wrapper expression: a field with a
    /// recorded single-generic inner, a binding that inherited one, or
    /// a `clone` of either — references and derefs stripped.
    fn wrapped_of(&self, expression: &syn::Expr) -> Option<String> {
        match strip_ref_deref(expression) {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.local_wraps.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => {
                let owner = self.expr_type(&field.base)?;
                self.wrap(&format!("{owner}.{}", member_name(&field.member)))
                    .cloned()
            }
            syn::Expr::MethodCall(call) if call.method == "clone" => {
                self.wrapped_of(&call.receiver)
            }
            _ => None
        }
    }

    /// Types the parameter of a single-parameter closure argument with
    /// the receiver field's wrapped inner type — the `do_io(|inner|
    /// inner.read(buf))` shape, where the wrapper hands its wrapped
    /// value to the closure. The same class of approximation as the
    /// iterator-adapter element binding, and wrong the same way a
    /// combinator passing something other than the wrapped value would
    /// make it; single-generic wrappers keep that surface small.
    fn bind_wrapped_closure(&mut self, call: &syn::ExprMethodCall) {
        let Some(inner) = self.wrapped_of(&call.receiver) else {
            return;
        };
        for argument in &call.args {
            if let syn::Expr::Closure(closure) = argument
                && closure.inputs.len() == 1
                && let Some(pattern) = closure.inputs.first()
                && let Some(name) = binding_name(pattern)
            {
                self.param_types.insert(name, inner.clone());
            }
        }
    }

    /// Normalizes a call expression into its return-table key, seeing
    /// through `.await` and preferring the owner-qualified associated
    /// key over the bare free-function name.
    fn call_key(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Await(awaited) => self.call_key(&awaited.base),
            syn::Expr::Call(call) => {
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let segments = &path.path.segments;
                let name = segments.last()?.ident.to_string();
                if segments.len() >= 2 {
                    let written = segments[segments.len() - 2].ident.to_string();
                    let owner = if written == "Self" {
                        self.param_types.get("self").cloned().unwrap_or(written)
                    } else {
                        written
                    };
                    let qualified = format!("{owner}::{name}");
                    if self.ret(&qualified).is_some() {
                        return Some(qualified);
                    }
                }
                self.ret(&name).is_some().then_some(name)
            }
            _ => None
        }
    }

    /// Types any expression the tables can settle: bindings and fields
    /// by their recorded types, indexed collections by element, calls
    /// through [`Self::called_return`].
    fn expr_type(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map(|name| self.bounds.get(name).unwrap_or(name).clone()),
            syn::Expr::Field(field) => {
                let owner = self.expr_type(&field.base)?;
                self.field(&format!("{owner}.{}", member_name(&field.member)))
                    .map(|inner| self.bounds.get(inner).unwrap_or(inner).clone())
            }
            syn::Expr::Index(indexed) => self.element_of(&indexed.expr),
            other => self.called_return(other)
        }
    }

    /// Types an expression that calls a function with a known return: a
    /// `?`- or `unwrap`/`expect`-unwrapped call yields the success
    /// type, a plain call the written return type, a method chain
    /// resolves receiver-first, and a well-known external constructor
    /// keeps its external type name so calls on it attribute outside.
    fn called_return(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Try(tried) => self
                .call_key(&tried.expr)
                .and_then(|key| self.ret(&key))
                .and_then(|returned| returned.success.clone())
                .or_else(|| self.optional_element(&tried.expr)),
            syn::Expr::MethodCall(call) if call.method == "unwrap" || call.method == "expect" => {
                if let Some(key) = self.call_key(&call.receiver) {
                    return self.ret(&key).and_then(|returned| returned.success.clone());
                }
                self.optional_element(&call.receiver)
                    .or_else(|| initializer_type(&call.receiver))
                    .or_else(|| self.called_return(&call.receiver))
            }
            syn::Expr::Await(awaited) => self.called_return(&awaited.base),
            syn::Expr::MethodCall(call) => {
                let owner = self.expr_type(&call.receiver)?;
                let method = call.method.to_string();
                if method == "clone" {
                    return Some(owner);
                }
                let declared = self
                    .ret(&format!("{owner}::{method}"))
                    .map(|returned| returned.plain.clone());
                if knowledge::PROJECTION_METHODS.contains(&method.as_str())
                    && declared
                        .as_deref()
                        .is_none_or(|view| knowledge::PROJECTION_VIEWS.contains(&view))
                {
                    return Some(owner);
                }
                declared.or_else(|| knowledge::method_return(&owner, &method).map(str::to_owned))
            }
            syn::Expr::Call(call) => {
                if let Some(inner) = pinned_inner(call) {
                    return self.expr_type(inner);
                }
                if let Some(key) = self.call_key(expression) {
                    return self.ret(&key).map(|returned| returned.plain.clone());
                }
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let name = path.path.segments.last()?.ident.to_string();
                EXTERNAL_CONSTRUCTORS
                    .iter()
                    .find(|(constructor, _)| *constructor == name)
                    .map(|(_, produced)| (*produced).to_owned())
                    .or_else(|| knowledge::constructor_type(&name).map(str::to_owned))
            }
            _ => self
                .call_key(expression)
                .and_then(|key| self.ret(&key))
                .map(|returned| returned.plain.clone())
        }
    }

    /// Renders a path as segment names, the first segment substituted
    /// with its full `use` path when the file imported it — so a bare
    /// `remove_dir_all` imported from `std::fs` classifies as a
    /// filesystem sink.
    fn expanded_segments(&self, path: &syn::Path) -> Vec<String> {
        let mut segments: Vec<String> = Vec::new();
        for (position, segment) in path.segments.iter().enumerate() {
            let name = segment.ident.to_string();
            if position == 0 {
                match self.imports.get(&name) {
                    Some(full) => segments.extend(full.split("::").map(str::to_owned)),
                    None => segments.push(name)
                }
            } else {
                segments.push(name);
            }
        }
        segments
    }

    /// Classifies a call path against the effect sink patterns, seen
    /// through the file's imports.
    fn record_path_effect(&mut self, path: &syn::Path) {
        let segments = self.expanded_segments(path);
        let joined = segments.join("::");
        let effect = if joined.contains("fs::") || segments.first().is_some_and(|s| s == "File") {
            Some(Effect::FileSystem)
        } else if joined.contains("net::")
            || segments
                .iter()
                .any(|s| s == "TcpStream" || s == "TcpListener" || s == "UdpSocket")
        {
            Some(Effect::Network)
        } else if joined.contains("process::") || segments.first().is_some_and(|s| s == "Command")
        {
            Some(Effect::Process)
        } else if joined.contains("env::") {
            Some(Effect::Environment)
        } else if segments.len() == 2 {
            segments
                .first()
                .and_then(|first| knowledge::crate_effect(first))
        } else {
            None
        };
        if let Some(effect) = effect {
            self.facts.effects.push(effect);
        }
    }

    /// Visits `body` with one more enclosing construct on the stack, so
    /// every write recorded inside it carries that construct.
    fn under<F: FnOnce(&mut Self)>(&mut self, guard: super::output::Guard, body: F) {
        self.guards.push(guard);
        body(self);
        self.guards.pop();
    }

    /// Snapshot of everything standing between the current position and
    /// unconditional execution.
    fn site_guard(&self) -> super::output::SiteGuard {
        super::output::SiteGuard {
            guards:      self.guards.clone(),
            prior_exits: self.prior_exits,
            dead:        self.dead
        }
    }

    /// Records one write to an output stream, in body order.
    ///
    /// The payload is whatever the invocation proves and no more; the
    /// byte range of the invocation is kept so a claim can point at the
    /// construct that produced it.
    fn record_output(
        &mut self,
        invocation: &syn::Macro,
        stream: super::output::Stream,
        newline: super::output::Newline,
        debug_macro: bool
    ) {
        use syn::spanned::Spanned;
        let span = invocation.span().byte_range();
        self.facts.output_writes.push(super::output::OutputWrite {
            stream,
            payload: super::output::macro_payload(invocation, debug_macro),
            newline,
            site: rustmanifest_schema::Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            },
            guard: self.site_guard(),
            multiplicity: if self.guards.iter().any(|guard| {
                matches!(
                    guard,
                    super::output::Guard::For | super::output::Guard::While
                )
            }) {
                super::output::Multiplicity::Unbounded
            } else {
                super::output::Multiplicity::Once
            }
        });
    }

    /// Records one normalized call candidate, resolving `Self` and typed
    /// receivers through the local type map.
    fn record_call(&mut self, owner: Option<&str>, callee: &str) {
        let candidate = match owner {
            None => callee.to_owned(),
            Some("Self") => self.param_types.get("self").map_or_else(
                || format!("Self::{callee}"),
                |own| format!("{own}::{callee}")
            ),
            Some(owner) => format!("{owner}::{callee}")
        };
        self.facts.call_sites.push(super::types::CallSite {
            candidate: candidate.clone(),
            guard:     self.site_guard()
        });
        self.facts.calls.push(candidate);
    }
}

/// Inner expression of a `Pin::new(&mut *value)` call: `Pin` is a
/// transparent pointer for receiver typing, so the pinned value's type
/// is the wrapped expression's type, references and derefs stripped.
fn pinned_inner(call: &syn::ExprCall) -> Option<&syn::Expr> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let segments = &path.path.segments;
    if segments.len() < 2
        || segments[segments.len() - 1].ident != "new"
        || segments[segments.len() - 2].ident != "Pin"
    {
        return None;
    }
    call.args.first().map(strip_ref_deref)
}

/// Strips leading references and dereferences off an expression.
fn strip_ref_deref(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(reference) => strip_ref_deref(&reference.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            strip_ref_deref(&unary.expr)
        }
        _ => expression
    }
}

/// Table key of a field member: the written name, or the position for
/// the unnamed fields of a tuple struct — the shape newtype wrappers
/// carry their payload in. Used by the typing channel only; the
/// read/write fact channel keeps naming its members as written.
fn member_name(member: &syn::Member) -> String {
    match member {
        syn::Member::Named(name) => name.to_string(),
        syn::Member::Unnamed(index) => index.index.to_string()
    }
}

/// Name of a single binding pattern, seen through references and type
/// ascriptions.
fn binding_name(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Reference(reference) => binding_name(&reference.pat),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        _ => None
    }
}

/// Infers the type name of a `let` initializer: a struct literal or a
/// `Type::new(..)` / `Type::default()` constructor call.
fn initializer_type(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Struct(literal) => literal
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let segments = &path.path.segments;
            if segments.len() < 2 {
                return None;
            }
            let method = segments.last()?.ident.to_string();
            if method != "new" && method != "default" {
                return None;
            }
            Some(segments[segments.len() - 2].ident.to_string())
        }
        syn::Expr::Reference(reference) => initializer_type(&reference.expr),
        _ => None
    }
}

impl<'ast> Visit<'ast> for FactCollector {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            let segments = &path.path.segments;
            if segments.len() == 1 {
                self.record_call(None, &segments[0].ident.to_string());
            } else if segments.len() >= 2 {
                let owner = segments[segments.len() - 2].ident.to_string();
                let callee = segments[segments.len() - 1].ident.to_string();
                self.record_call(Some(&owner), &callee);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let name = match &local.pat {
            syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
            syn::Pat::Type(typed) => {
                if let syn::Pat::Ident(pattern) = typed.pat.as_ref() {
                    if let Some(type_name) = type_last_segment(&typed.ty) {
                        self.param_types
                            .insert(pattern.ident.to_string(), type_name);
                    }
                    if let Some(element) = element_type(&typed.ty) {
                        self.elem_types.insert(pattern.ident.to_string(), element);
                    }
                    Some(pattern.ident.to_string())
                } else {
                    None
                }
            }
            _ => None
        };
        if let (Some(name), init) = (name.clone(), &local.init)
            && let Some(inner) = init.as_ref().and_then(|init| self.wrapped_of(&init.expr))
        {
            self.local_wraps.insert(name, inner);
        }
        if let (Some(name), init) = (name, &local.init) {
            let constructed = init.as_ref().and_then(|init| {
                initializer_type(&init.expr).or_else(|| self.called_return(&init.expr))
            });
            match constructed {
                Some(type_name) => {
                    self.param_types.insert(name.clone(), type_name);
                    self.fresh.insert(name);
                }
                None => {
                    self.fresh.remove(&name);
                }
            }
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_for_loop(&mut self, loop_expr: &'ast syn::ExprForLoop) {
        self.bind_element(&loop_expr.pat, &loop_expr.expr);
        self.visit_expr(&loop_expr.expr);
        self.under(super::output::Guard::For, |collector| {
            collector.visit_block(&loop_expr.body);
        });
    }

    fn visit_expr_while(&mut self, loop_expr: &'ast syn::ExprWhile) {
        self.visit_expr(&loop_expr.cond);
        self.under(super::output::Guard::While, |collector| {
            collector.visit_block(&loop_expr.body);
        });
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.visit_expr(&conditional.cond);
        self.under(super::output::Guard::If, |collector| {
            collector.visit_block(&conditional.then_branch);
        });
        if let Some((_, otherwise)) = &conditional.else_branch {
            self.under(super::output::Guard::Else, |collector| {
                collector.visit_expr(otherwise);
            });
        }
    }

    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        self.visit_expr(&matched.expr);
        for arm in &matched.arms {
            let guard = super::output::Guard::MatchArm {
                irrefutable: matches!(arm.pat, syn::Pat::Wild(_) | syn::Pat::Ident(_)),
                guarded:     matches!(arm.pat, syn::Pat::Guard(_))
            };
            self.under(guard, |collector| {
                collector.visit_arm(arm);
            });
        }
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.under(super::output::Guard::Closure, |collector| {
            syn::visit::visit_expr_closure(collector, closure);
        });
    }

    fn visit_item_fn(&mut self, nested: &'ast syn::ItemFn) {
        self.under(super::output::Guard::NestedItem, |collector| {
            syn::visit::visit_item_fn(collector, nested);
        });
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let entry_exits = self.prior_exits;
        let entry_dead = self.dead;
        for statement in &block.stmts {
            if gated(statement) {
                self.under(super::output::Guard::CfgGated, |collector| {
                    collector.visit_stmt(statement);
                });
            } else {
                self.visit_stmt(statement);
            }
            if diverges(statement) {
                self.dead = true;
            } else if exits(statement) {
                self.prior_exits += 1;
            }
        }
        self.dead = entry_dead;
        self.prior_exits = entry_exits;
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.record_write_target(&assignment.left);
        self.visit_expr(&assignment.right);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.record_write_target(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if let Some(key) = self.field_key(field) {
            self.facts.reads.push(key);
        }
        if let syn::Expr::Path(path) = field.base.as_ref()
            && let Some(ident) = path.path.get_ident()
        {
            let named = matches!(field.member, syn::Member::Named(_));
            self.bump_param(&ident.to_string(), named);
            return;
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if MUTATING_METHODS.contains(&method.as_str())
            && let syn::Expr::Field(field) = call.receiver.as_ref()
            && let Some(key) = self.field_key(field)
        {
            self.record_field_write(field, key);
        }
        if method == "clone" || method == "to_owned" || method == "to_string" {
            self.facts.clones += 1;
        }
        if ITERATOR_ADAPTERS.contains(&method.as_str()) {
            for argument in &call.args {
                if let syn::Expr::Closure(closure) = argument
                    && closure.inputs.len() == 1
                    && let Some(pattern) = closure.inputs.first()
                {
                    self.bind_element(pattern, &call.receiver);
                }
            }
        } else {
            self.bind_wrapped_closure(call);
        }
        if method == knowledge::SERVICE_CALL
            && let Some(inner) = self.wrapped_of(&call.receiver)
        {
            self.record_call(Some(&inner), &method);
        }
        match self.expr_type(&call.receiver) {
            Some(owner) => {
                if let Some(effect) = knowledge::method_effect(&owner, &method)
                    .or_else(|| crate::semantic::std_knowledge::method_effect(&owner, &method))
                {
                    self.facts.effects.push(effect);
                }
                self.record_call(Some(&owner), &method);
            }
            None => self.record_call(Some("?"), &method)
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_struct(&mut self, literal: &'ast syn::ExprStruct) {
        if let Some(segment) = literal.path.segments.last() {
            let name = segment.ident.to_string();
            let constructed = if name == "Self" {
                self.param_types.get("self").cloned().unwrap_or(name)
            } else {
                name
            };
            self.facts.constructs.push(constructed);
        }
        syn::visit::visit_expr_struct(self, literal);
    }

    fn visit_expr_unsafe(&mut self, block: &'ast syn::ExprUnsafe) {
        self.facts.effects.push(Effect::Unsafe);
        syn::visit::visit_expr_unsafe(self, block);
    }

    fn visit_expr_try(&mut self, tried: &'ast syn::ExprTry) {
        if let [enclosing] = self.facts.error_types.clone().as_slice()
            && let Some(key) = self.call_key(&tried.expr)
            && let Some(callee_error) = self.ret(&key).and_then(|r| r.error.clone())
            && callee_error != *enclosing
        {
            self.record_call(Some(enclosing), "from");
        }
        syn::visit::visit_expr_try(self, tried);
    }

    fn visit_expr_await(&mut self, awaited: &'ast syn::ExprAwait) {
        if let Some(future_type) = self.called_return(&awaited.base)
            && self.ret(&format!("{future_type}::poll")).is_some()
        {
            self.record_call(Some(&future_type), "poll");
        }
        syn::visit::visit_expr_await(self, awaited);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.path.segments.len() >= 2 {
            self.record_path_effect(&path.path);
            let first = path.path.segments[0].ident.to_string();
            let last = path.path.segments.last().map(|s| s.ident.to_string());
            if matches!(
                (first.as_str(), last.as_deref()),
                (
                    "Box" | "Vec" | "String" | "Rc" | "Arc",
                    Some("new" | "from" | "with_capacity")
                )
            ) {
                self.facts.allocations += 1;
            }
        } else if let Some(ident) = path.path.get_ident() {
            let name = ident.to_string();
            if self.imports.contains_key(&name) {
                self.record_path_effect(&path.path);
            }
            self.bump_param(&name, false);
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if let Some(segment) = invocation.path.segments.last() {
            let name = segment.ident.to_string();
            if let Some((_, stream, newline)) = OUTPUT_MACROS
                .iter()
                .find(|(macro_name, _, _)| *macro_name == name)
            {
                self.facts.effects.push(Effect::ConsoleIo);
                self.record_output(invocation, *stream, *newline, name == "dbg");
            }
            if name == "vec" || name == "format" {
                self.facts.allocations += 1;
            }
            if let Some(effect) =
                knowledge::sql_macro_effect(&name, &invocation.tokens.to_string())
            {
                self.facts.effects.push(effect);
            }
        }
        syn::visit::visit_macro(self, invocation);
    }
}
