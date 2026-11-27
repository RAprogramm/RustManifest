// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Classification of how a loop-local `let` binding obtained its value.
//!
//! A binding is interesting to the loop checks only when its initializer
//! is a clone of an existing place or a fresh container allocation; every
//! other initializer is ignored. Adding a new origin kind means adding a
//! variant here and a classifier function beside the existing two.
//!
//! Both classifications are read off what the compiler settled about the
//! call and never off the name that was written. A copy is a call that
//! borrows a place and hands back the very type it borrowed; an
//! allocation is a call into the allocating crate with nothing to be
//! called on. Written names answered both before, and answered for every
//! method a workspace happened to spell the same way.

use rustmanifest_schema::{ResolvedCall, ResolvedMode};

use crate::semantic::expr::place_chain;

/// Crate the language's own heap containers are declared in, which is
/// what makes a call into it an allocation.
const ALLOCATING_CRATE: &str = "alloc";

/// How a loop-local `let` binding obtained its value.
pub(super) enum Origin {
    /// A call that borrows a place and hands its type back.
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

/// Classifies `expr` as a copy of a stable or unstable place, from the
/// call the compiler settled at it.
///
/// A copy is not a spelling: it is a call taking nothing, borrowing the
/// place it is written on, and handing back that place's own type. A call
/// the resolver left unsettled is classified as nothing at all.
pub(super) fn clone_origin(
    expr: &syn::Expr,
    settled: Option<&ResolvedCall>,
    bound: &[String],
    mutated: &[String]
) -> Option<Origin> {
    let syn::Expr::MethodCall(call) = expr else {
        return None;
    };
    let settled = settled?;
    if !call.args.is_empty() || settled.takes != Some(ResolvedMode::Borrowed) {
        return None;
    }
    let borrowed = settled.receiver.as_ref()?.trim_start_matches('&').trim();
    if borrowed.strip_prefix("mut ").unwrap_or(borrowed) != settled.returns {
        return None;
    }
    let method = settled.callee.clone();
    let chain = place_chain(&call.receiver)?;
    let root = chain.split('.').next().unwrap_or(&chain).to_owned();
    let stable = !bound.contains(&root) && !mutated.contains(&root);
    Some(Origin::Clone {
        chain,
        method,
        stable
    })
}

/// Classifies `expr` as a fresh heap allocation whose arguments do not
/// depend on loop-bound values.
///
/// What allocates is the crate the reached function is declared in: the
/// language's heap containers all live in one, and a call into it with
/// nothing to be called on hands back a freshly allocated value. Which
/// two type names somebody wrote down decided this before, and decided it
/// for every namesake as well.
pub(super) fn allocation_origin(
    expr: &syn::Expr,
    settled: Option<&ResolvedCall>,
    reached_macro: Option<&str>,
    bound: &[String]
) -> Option<Origin> {
    match expr {
        syn::Expr::Call(call) => {
            let settled = settled?;
            let reached = settled.reaches.as_ref()?;
            if settled.receiver.is_some()
                || reached.split("::").next() != Some(ALLOCATING_CRATE)
                || !call.args.iter().all(|arg| invariant_argument(arg, bound))
            {
                return None;
            }
            Some(Origin::Allocation {
                call: reached.clone()
            })
        }
        syn::Expr::Macro(invocation)
            if invocation.mac.tokens.is_empty()
                && reached_macro.and_then(|named| named.split("::").next())
                    == Some(ALLOCATING_CRATE) =>
        {
            Some(Origin::Allocation {
                call: reached_macro.unwrap_or_default().to_owned()
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
