// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Classification of how a loop-local `let` binding obtained its value.
//!
//! A binding is interesting to the loop checks only when its initializer
//! is a clone of an existing place or a fresh container allocation; every
//! other initializer is ignored. Adding a new origin kind means adding a
//! variant here and a classifier function beside the existing two.

use crate::semantic::expr::place_chain;

/// How a loop-local `let` binding obtained its value.
pub(super) enum Origin {
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

/// Classifies `expr` as a clone of a stable or unstable place.
pub(super) fn clone_origin(
    expr: &syn::Expr,
    bound: &[String],
    mutated: &[String]
) -> Option<Origin> {
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
pub(super) fn allocation_origin(expr: &syn::Expr, bound: &[String]) -> Option<Origin> {
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
