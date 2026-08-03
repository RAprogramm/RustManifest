// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The proving expressions a derivation looks for inside a body.
//!
//! A property of the print path is only claimed when the library's own
//! body contains the expression that establishes it: the lock the handle
//! takes, the call into the formatting machinery, the panic on a failed
//! write. Each of them is named as a kind here and located by a single
//! visitor, which records the first match and its byte range so the fact
//! built from it cites the library rather than this repository.

use rustmanifest_schema::Range;
use syn::spanned::Spanned;

use super::citation::byte_range;

/// Which proving expression to locate inside a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionKind {
    /// A method call named `lock`.
    LockCall,
    /// A method call named `write_fmt`.
    FormatWrite,
    /// A panic-family macro invocation.
    Panic
}

/// First proving expression of one kind inside a body.
pub(super) fn locate(block: &syn::Block, kind: ExpressionKind) -> Option<Range> {
    let mut scan = Locate {
        kind,
        found: None
    };
    syn::visit::Visit::visit_block(&mut scan, block);
    scan.found
}

/// Visitor locating one proving expression.
struct Locate {
    /// Expression being looked for.
    kind:  ExpressionKind,
    /// Byte range of the first match.
    found: Option<Range>
}

impl Locate {
    /// Records the first match only.
    fn note(&mut self, span: &std::ops::Range<usize>) {
        if self.found.is_none() {
            self.found = Some(byte_range(span));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for Locate {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let wanted = match self.kind {
            ExpressionKind::LockCall => "lock",
            ExpressionKind::FormatWrite => "write_fmt",
            ExpressionKind::Panic => ""
        };
        if !wanted.is_empty() && call.method == wanted {
            self.note(&call.span().byte_range());
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if self.kind == ExpressionKind::Panic
            && invocation
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "panic")
        {
            self.note(&invocation.span().byte_range());
        }
        syn::visit::visit_macro(self, invocation);
    }
}
