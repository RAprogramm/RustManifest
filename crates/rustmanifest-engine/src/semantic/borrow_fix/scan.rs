// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the scan knows: the names it looks for, the evidence it has
//! gathered so far, and the two recordings that turn a matched signature
//! or a matched call into an edit.
//!
//! The traversal that feeds it lives in [`walk`].

mod walk;

use std::path::PathBuf;

use rustmanifest_schema::Fix;
use syn::spanned::Spanned;

use super::edit::insertion;

/// Single-pass scanner over one file at a time.
pub(super) struct Scan<'names> {
    /// Bare function name being borrowed.
    pub(super) bare:           &'names str,
    /// Impl owner for methods, `None` for free functions.
    pub(super) owner:          Option<&'names str>,
    /// Parameter name, `self` included.
    pub(super) param:          &'names str,
    /// Index of the source currently visited.
    pub(super) source_index:   usize,
    /// Path of the source currently visited.
    pub(super) current_path:   PathBuf,
    /// Phase switch: definitions are recorded first, calls second.
    pub(super) collect_calls:  bool,
    /// Every function or method definition matching the bare name.
    pub(super) definitions:    Vec<usize>,
    /// The signature edit, once the target definition is seen.
    pub(super) signature_edit: Option<Fix>,
    /// Zero-based position of the parameter among call arguments;
    /// `None` for a `self` receiver (call sites auto-ref).
    pub(super) param_position: Option<usize>,
    /// One `&`-insertion per located call argument.
    pub(super) call_edits:     Vec<Fix>,
    /// Bare-path references outside call position; any of them aborts.
    pub(super) bare_path_refs: usize
}

impl Scan<'_> {
    /// Records the signature edit for a matching definition.
    fn record_definition(&mut self, signature: &syn::Signature) {
        self.definitions.push(self.source_index);
        for (position, input) in signature.inputs.iter().enumerate() {
            match input {
                syn::FnArg::Receiver(receiver)
                    if self.param == "self"
                        && !matches!(receiver.kind, syn::ReceiverKind::Reference(..)) =>
                {
                    let bytes = receiver.span().byte_range();
                    self.signature_edit = Some(insertion(bytes.start, "&", None));
                }
                syn::FnArg::Typed(argument) => {
                    let syn::Pat::Ident(pattern) = argument.pat.as_ref() else {
                        continue;
                    };
                    if pattern.ident != self.param {
                        continue;
                    }
                    let bytes = argument.ty.span().byte_range();
                    self.signature_edit = Some(insertion(bytes.start, "&", None));
                    let receiver_shift = usize::from(matches!(
                        signature.inputs.first(),
                        Some(syn::FnArg::Receiver(_))
                    ));
                    self.param_position = Some(position - receiver_shift);
                }
                syn::FnArg::Receiver(_) => {}
            }
        }
    }

    /// Records an `&`-insertion for the matching argument of a call.
    fn record_call(&mut self, arguments: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>) {
        let Some(position) = self.param_position else {
            return;
        };
        if let Some(argument) = arguments.iter().nth(position) {
            if matches!(argument, syn::Expr::Reference(_)) {
                return;
            }
            let bytes = argument.span().byte_range();
            self.call_edits
                .push(insertion(bytes.start, "&", Some(self.current_path.clone())));
        }
    }
}
