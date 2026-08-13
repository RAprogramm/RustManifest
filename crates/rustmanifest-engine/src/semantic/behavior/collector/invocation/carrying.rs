// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Handing on the origin a call carries rather than derives.
//!
//! A library wrapper and a library method that answers with one of its
//! own inputs both stand for a value written somewhere else. Naming the
//! call would put a step between a value and itself, and a term full of
//! those steps says less with every one of them — so the origin written
//! inside is handed on, and the call is not named at all.

use super::super::FactCollector;
use crate::semantic::{behavior::Provenance, std_knowledge::Carried};

/// The crate whose outcome variants wrap one value and add nothing to
/// it.
///
/// Which three names those variants have is not the question — a
/// workspace variant spelled `Ok` is a different item, and the compiler
/// is the one that tells them apart. Three written names answered here
/// before, and answered for every namesake too.
const OUTCOME_CRATE: &str = "core";

impl FactCollector {
    /// Whether the written path names an outcome variant of the crate
    /// that declares them, as the compiler settled it.
    fn wraps_one_value(&self, path: &syn::Path) -> bool {
        use syn::spanned::Spanned as _;

        let span = path.span().byte_range();
        let (Ok(start), Ok(end)) = (u32::try_from(span.start), u32::try_from(span.end)) else {
            return false;
        };
        if self.items.get(&(start, end)).map(String::as_str) != Some("variant") {
            return false;
        }
        self.named
            .get(&(start, end))
            .and_then(|reached| reached.split("::").next())
            == Some(OUTCOME_CRATE)
    }

    /// Hands on the origin of the one value a library wrapper is built
    /// around.
    pub(in crate::semantic::behavior::collector) fn carry_wrapper(
        &mut self,
        call: &syn::ExprCall,
        span: &std::ops::Range<usize>
    ) {
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return;
        };
        if !self.wraps_one_value(&path.path) {
            return;
        }
        let [only] = call.args.iter().collect::<Vec<_>>()[..] else {
            return;
        };
        let carried = self.origin_of(only);
        self.produced.insert(
            (
                u32::try_from(span.start).unwrap_or(u32::MAX),
                u32::try_from(span.end).unwrap_or(u32::MAX)
            ),
            carried
        );
    }

    /// Hands on the origin a library call carries, where a row says which
    /// of its inputs it carries.
    ///
    /// A method whose answer is derived rather than carried has no row and
    /// is not seen through.
    pub(in crate::semantic::behavior::collector) fn carry_outcome(
        &mut self,
        call: &syn::ExprMethodCall,
        span: &std::ops::Range<usize>
    ) {
        let method = call.method.to_string();
        let Some(carried) = crate::semantic::std_knowledge::carried_by(&method) else {
            return;
        };
        let origin = match carried {
            Carried::Receiver | Carried::Held => self.origin_of(&call.receiver),
            Carried::HeldOrArgument(position) => {
                let held = self.origin_of(&call.receiver);
                match call.args.iter().nth(position.into()) {
                    Some(argument) => {
                        let fallback = self.origin_of(argument);
                        self.either(span, held, fallback)
                    }
                    None => held
                }
            }
            Carried::HeldOrDefault => {
                let held = self.origin_of(&call.receiver);
                self.either(span, held, Provenance::Defaulted)
            }
            Carried::ClosureOver(position) => {
                let Some(syn::Expr::Closure(closure)) = call.args.iter().nth(position.into())
                else {
                    return;
                };
                self.through_closure(&call.receiver, closure)
            }
        };
        let at = (
            u32::try_from(span.start).unwrap_or(u32::MAX),
            u32::try_from(span.end).unwrap_or(u32::MAX)
        );
        if matches!(self.produced.get(&at), Some(Provenance::Read(_))) {
            return;
        }
        self.produced.insert(at, origin);
    }

    /// Reads what a closure hands back, with its own input standing for
    /// what the receiver holds.
    ///
    /// The binding lives exactly as long as the reading of the body: a
    /// name written inside a closure is not the same name written outside
    /// it, and leaving it behind would answer a later reading with a value
    /// from a scope it never saw.
    fn through_closure(&mut self, receiver: &syn::Expr, closure: &syn::ExprClosure) -> Provenance {
        let held = self.origin_of(receiver);
        let restored = closure_input(closure).as_ref().map(|name| {
            (
                name.clone(),
                self.origins.insert(name.clone(), held.clone())
            )
        });
        let answer = self.origin_of(&closure.body);
        if let Some((name, previous)) = restored {
            match previous {
                Some(origin) => self.origins.insert(name, origin),
                None => self.origins.remove(&name)
            };
        }
        answer
    }
}

/// The single plain name a closure binds its input to, where it binds one.
///
/// A pattern taking the input apart binds several names to parts of it,
/// and which part each name holds is a question this tier does not answer.
pub(in crate::semantic::behavior::collector) fn closure_input(
    closure: &syn::ExprClosure
) -> Option<String> {
    match closure.inputs.first() {
        Some(syn::Pat::Ident(named)) if named.subpat.is_none() => Some(named.ident.to_string()),
        _ => None
    }
}
