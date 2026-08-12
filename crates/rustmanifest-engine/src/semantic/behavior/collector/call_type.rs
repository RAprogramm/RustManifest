// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Call typing: the return-table key a call expression normalizes to, and
//! the type name that call hands back once the unwrapping written around
//! it is read.

use super::{FactCollector, typing::initializer_type, unwrapping::pinned_inner};
use crate::semantic::{knowledge, std_knowledge::EXTERNAL_CONSTRUCTORS};

/// Methods whose written type argument is the type they hand back, so
/// the turbofish beside the call is the whole answer.
///
/// Each of these is declared to produce exactly its own single type
/// parameter — nothing is wrapped around it — which is why the written
/// argument can be read as the result. A method that wraps what it
/// produces, the way parsing wraps in a fallible, is not on the list.
const TURBOFISH_RESULTS: [&str; 3] = ["collect", "sum", "product"];

/// The type a call states outright through its own turbofish.
fn turbofished(call: &syn::ExprMethodCall) -> Option<String> {
    if !TURBOFISH_RESULTS.contains(&call.method.to_string().as_str()) {
        return None;
    }
    let written = call.turbofish.as_ref()?;
    let mut types = written.args.iter().filter_map(|argument| match argument {
        syn::GenericArgument::Type(held) => Some(held),
        _ => None
    });
    let (Some(only), None) = (types.next(), types.next()) else {
        return None;
    };
    crate::semantic::behavior::signature::type_last_segment(only)
}

impl FactCollector {
    /// The type a resolver settled for the receiver of the call at this
    /// position.
    ///
    /// The resolver answers what written text cannot — which function a
    /// call reaches and what it hands back — and its answer outranks
    /// every reading below, which infers from spelling alone. A position
    /// the resolver left out is absent from the overlay, so the reading
    /// falls through to what the source itself proves.
    pub(super) fn settled_receiver(&self, expression: &syn::Expr) -> Option<String> {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.receivers.get(&(start, end)).cloned()
    }

    /// The type a resolver settled for the expression at this position.
    fn settled_type(&self, expression: &syn::Expr) -> Option<String> {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.settled.get(&(start, end)).cloned()
    }

    /// Normalizes a call expression into its return-table key, seeing
    /// through `.await` and preferring the owner-qualified associated
    /// key over the bare free-function name.
    pub(super) fn call_key(&self, expression: &syn::Expr) -> Option<String> {
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

    /// Types an expression that calls a function with a known return: a
    /// `?`- or `unwrap`/`expect`-unwrapped call yields the success
    /// type, a plain call the written return type, a method chain
    /// resolves receiver-first, and a well-known external constructor
    /// keeps its external type name so calls on it attribute outside.
    pub(super) fn called_return(&self, expression: &syn::Expr) -> Option<String> {
        if let Some(settled) = self.settled_type(expression) {
            return Some(settled);
        }
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
                if let Some(written) = turbofished(call) {
                    return Some(written);
                }
                let owner = self
                    .settled_receiver(expression)
                    .or_else(|| self.expr_type(&call.receiver))?;
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
}
