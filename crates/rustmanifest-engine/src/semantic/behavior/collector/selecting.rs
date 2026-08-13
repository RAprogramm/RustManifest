// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether a call takes one element out of what it is written on.
//!
//! `map.get(&key)` is not an opaque call: the value it hands back is an
//! element of the receiver, and the compiler says so in the two
//! renderings it settles — the call answers with an option carrying a
//! type the receiver itself carries. Five method names answered this
//! before, and answered for a workspace collection spelling its lookup
//! anything else not at all.
//!
//! What this does **not** state is which element. That depends on what
//! is in the collection at the time, which no reading of the source
//! settles; the operation says the value came out of this collection,
//! and stops there.

use super::FactCollector;

/// The rendering an answer takes when the collection may hold nothing
/// under the key it was asked with.
const OPTIONAL: &str = "Option";

/// The type a written position is counted in.
const POSITION: &str = "usize";

impl FactCollector {
    /// Whether the call hands back an element of what it is written on,
    /// as the two types the compiler settled say.
    pub(super) fn selects_an_element(&self, call: &syn::ExprMethodCall) -> bool {
        use syn::spanned::Spanned as _;

        let held = self.settled_expr_type_full(&call.receiver);
        let Some(held) = held else {
            return false;
        };
        let handed = self.settled_type_over_full(&call.span().byte_range());
        let Some(handed) = handed else {
            return false;
        };
        let Some(carried) = optional_content(&handed) else {
            return false;
        };
        arguments_of(&held)
            .iter()
            .any(|argument| bare(argument) == carried)
    }

    /// Whether the call singles out one element of what it is written on
    /// by a written position: it is asked with a count and answers with
    /// something or nothing.
    ///
    /// Two written adapter names said this before, and said it for any
    /// namesake taking a number. A written number is read as one here
    /// because the resolver states no type for a literal: its type is
    /// not a question the source leaves open.
    pub(super) fn takes_element_at(&self, call: &syn::ExprMethodCall) -> bool {
        use syn::spanned::Spanned as _;

        let Some(handed) = self.settled_type_over_full(&call.span().byte_range()) else {
            return false;
        };
        if crate::semantic::behavior::outermost_name(handed.trim()).as_deref() != Some(OPTIONAL) {
            return false;
        }
        call.args.first().is_some_and(|argument| {
            self.settled_expr_type(argument).as_deref() == Some(POSITION)
                || matches!(
                    argument,
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(_),
                        ..
                    })
                )
        })
    }
}

/// The type an optional answer carries, where the answer is optional.
fn optional_content(rendered: &str) -> Option<String> {
    let trimmed = rendered.trim();
    if crate::semantic::behavior::outermost_name(trimmed).as_deref() != Some(OPTIONAL) {
        return None;
    }
    let inside = trimmed
        .split_once('<')
        .and_then(|(_, rest)| rest.strip_suffix('>'))?;
    Some(bare(inside))
}

/// A rendered type with the borrow and the mutability it is held
/// through dropped, so a borrowed element and an owned one are one
/// answer.
fn bare(rendered: &str) -> String {
    let trimmed = rendered.trim().trim_start_matches('&').trim();
    trimmed
        .strip_prefix("mut ")
        .unwrap_or(trimmed)
        .trim()
        .to_owned()
}

/// The top-level type arguments a rendered type carries.
fn arguments_of(rendered: &str) -> Vec<String> {
    let Some(inside) = rendered
        .trim()
        .split_once('<')
        .and_then(|(_, rest)| rest.strip_suffix('>'))
    else {
        return Vec::new();
    };
    let mut written = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (at, letter) in inside.char_indices() {
        match letter {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                written.push(inside[start..at].trim().to_owned());
                start = at + 1;
            }
            _ => {}
        }
    }
    written.push(inside[start..].trim().to_owned());
    written
}
