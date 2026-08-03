// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a match arm's pattern carries: the extra condition it holds, and
//! whether it accepts every value that reaches it.

/// The extra condition an arm carries, which its pattern holds: an arm
/// guard is a `Pat::Guard` wrapping the pattern it qualifies.
pub(super) fn arm_condition(pattern: &syn::Pat) -> Option<&syn::Expr> {
    match pattern {
        syn::Pat::Guard(guarded) => Some(&guarded.guard),
        _ => None
    }
}

/// Returns whether a pattern accepts every value reaching it, so an arm
/// carrying it binds rather than branches. A binding with a subpattern —
/// `n @ 1..=5` — is refutable: the subpattern decides, not the binding.
/// A guard is not part of this answer; it is recorded beside it.
pub(super) fn irrefutable(pattern: &syn::Pat) -> bool {
    match pattern {
        syn::Pat::Wild(_) => true,
        syn::Pat::Ident(binding) => binding.subpat.is_none(),
        syn::Pat::Guard(guarded) => irrefutable(&guarded.pat),
        syn::Pat::Paren(parenthesized) => irrefutable(&parenthesized.pat),
        syn::Pat::Reference(borrowed) => irrefutable(&borrowed.pat),
        syn::Pat::Type(typed) => irrefutable(&typed.pat),
        syn::Pat::Tuple(tuple) => tuple.elems.iter().all(irrefutable),
        _ => false
    }
}
