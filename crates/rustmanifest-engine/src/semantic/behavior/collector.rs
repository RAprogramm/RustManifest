// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Body analysis: the syntax visitor that accumulates behavioral facts —
//! reads, writes, constructions, effects, clones, and allocations — from
//! one function body against its signature-derived parameter types.

use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;

use super::{
    Returned,
    signature::type_last_segment,
    types::{Effect, FunctionFacts}
};
use crate::semantic::expr::{MUTATING_METHODS, compound_assignment};

/// Console-output macros classified as [`Effect::ConsoleIo`].
const CONSOLE_MACROS: [&str; 5] = ["dbg", "eprint", "eprintln", "print", "println"];

/// Syntax visitor accumulating facts from one function body.
pub(super) struct FactCollector {
    /// Facts collected so far.
    pub(super) facts:       FunctionFacts,
    /// Parameter name to type-name mapping from the signature.
    pub(super) param_types: BTreeMap<String, String>,
    /// Uniquely named workspace functions to their return types.
    pub(super) fn_returns:  BTreeMap<String, Returned>,
    /// Let bindings holding a value provably constructed in this body.
    pub(super) fresh:       BTreeSet<String>
}

impl FactCollector {
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
                        self.param_types
                            .get("self")
                            .cloned()
                            .unwrap_or(written)
                    } else {
                        written
                    };
                    let qualified = format!("{owner}::{name}");
                    if self.fn_returns.contains_key(&qualified) {
                        return Some(qualified);
                    }
                }
                self.fn_returns.contains_key(&name).then_some(name)
            }
            _ => None
        }
    }

    /// Types an expression that calls a uniquely named workspace
    /// function: a `?`-unwrapped call yields the success type, a plain
    /// call the written return type.
    fn called_return(&self, expression: &syn::Expr) -> Option<String> {
        if let syn::Expr::Try(tried) = expression {
            return self
                .call_key(&tried.expr)
                .and_then(|key| self.fn_returns.get(&key))
                .and_then(|returned| returned.success.clone());
        }
        self.call_key(expression)
            .and_then(|key| self.fn_returns.get(&key))
            .map(|returned| returned.plain.clone())
    }

    /// Classifies a call path against the effect sink patterns.
    fn record_path_effect(&mut self, path: &syn::Path) {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
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
        } else {
            None
        };
        if let Some(effect) = effect {
            self.facts.effects.push(effect);
        }
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
        self.facts.calls.push(candidate);
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
                    Some(pattern.ident.to_string())
                } else {
                    None
                }
            }
            _ => None
        };
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
        let receiver_type = match call.receiver.as_ref() {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .cloned(),
            other => self.called_return(other)
        };
        match receiver_type {
            Some(owner) => self.record_call(Some(&owner), &method),
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
            self.bump_param(&ident.to_string(), false);
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if let Some(ident) = invocation.path.get_ident() {
            let name = ident.to_string();
            if CONSOLE_MACROS.contains(&name.as_str()) {
                self.facts.effects.push(Effect::ConsoleIo);
            }
            if name == "vec" || name == "format" {
                self.facts.allocations += 1;
            }
        }
        syn::visit::visit_macro(self, invocation);
    }
}
