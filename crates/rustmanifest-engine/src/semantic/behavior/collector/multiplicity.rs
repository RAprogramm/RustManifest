// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How many times a write standing in a repeating construct's body runs,
//! read off the completion analysis of that body rather than stamped on
//! for standing in a loop.

use super::flow::Flow;
use crate::semantic::behavior::output::Multiplicity;

/// Whether a repeating construct's body provably leaves on its first
/// pass, so a write inside it runs no more than once per call of the
/// function around it.
///
/// The proof is the body's own completion: a body whose end is never
/// reached cannot start a second iteration. It survives only where
/// nothing can hand control back to the top of the loop — no `continue`
/// this loop absorbs, and no construct opaque enough to hide one.
fn leaves_on_first_pass(body: &Flow, label: Option<&syn::Label>) -> bool {
    matches!(body.completion, super::completion::Completion::Never)
        && !body.continues_into(label)
        && !body.opaque_jumps
}

/// How many times a write recorded inside a repeating construct's body
/// runs, read off the completion analysis of that body rather than
/// stamped on for standing in a loop.
pub(super) fn body_multiplicity(body: &Flow, label: Option<&syn::Label>) -> Multiplicity {
    if leaves_on_first_pass(body, label) {
        Multiplicity::Once
    } else {
        Multiplicity::Unbounded
    }
}
