// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fix synthesis for `borrowable-params`: turn an owned parameter into a
//! shared borrow and rewrite every caller — the cross-file fix a
//! single-site linter cannot do.
//!
//! The synthesis is gated hard; all of the following must hold, otherwise
//! no fix is produced and the finding stays a suggestion:
//!
//! - the function is private: every caller lives in the workspace;
//! - its bare name is unique among all functions, so call sites resolve without
//!   guessing;
//! - the function is never referenced as a bare path outside a call (no
//!   function pointers, no re-exports of the value);
//! - the parameter is a plain named binding whose type span is editable.
//!
//! Edits produced: `Type` → `&Type` in the signature, and `expr` →
//! `&expr` in the matching argument of every located call. A `self`
//! receiver needs only the signature edit — method-call sites auto-ref.

use std::path::PathBuf;

use rustmanifest_schema::{Applicability, Fix, Range};
use syn::{spanned::Spanned, visit::Visit};

use crate::source::Source;

/// Builds the borrow fixes for `function` (qualified name) and its
/// parameter `param`, or returns an empty list when safety cannot be
/// proven.
pub(super) fn borrow_fixes(
    sources: &[Source],
    function: &str,
    param: &str,
    public: bool
) -> Vec<Fix> {
    if public {
        return Vec::new();
    }
    let bare = function.rsplit("::").next().unwrap_or(function);
    let mut scan = Scan {
        bare,
        owner: function.rsplit_once("::").map(|(owner, _)| owner),
        param,
        source_index: 0,
        current_path: PathBuf::new(),
        collect_calls: false,
        definitions: Vec::new(),
        signature_edit: None,
        param_position: None,
        call_edits: Vec::new(),
        bare_path_refs: 0
    };
    for phase in [false, true] {
        scan.collect_calls = phase;
        for (index, source) in sources.iter().enumerate() {
            let Ok(file) = syn::parse_file(source.text()) else {
                continue;
            };
            scan.source_index = index;
            scan.current_path = source.path().to_path_buf();
            scan.visit_file(&file);
        }
    }
    if scan.definitions.len() != 1 || scan.bare_path_refs > 0 {
        return Vec::new();
    }
    let Some(signature_edit) = scan.signature_edit else {
        return Vec::new();
    };
    let mut fixes = vec![signature_edit];
    if scan.param_position.is_some() {
        fixes.extend(scan.call_edits);
    }
    fixes
}

/// Single-pass scanner over one file at a time.
struct Scan<'names> {
    /// Bare function name being borrowed.
    bare:           &'names str,
    /// Impl owner for methods, `None` for free functions.
    owner:          Option<&'names str>,
    /// Parameter name, `self` included.
    param:          &'names str,
    /// Index of the source currently visited.
    source_index:   usize,
    /// Path of the source currently visited.
    current_path:   PathBuf,
    /// Phase switch: definitions are recorded first, calls second.
    collect_calls:  bool,
    /// Every function or method definition matching the bare name.
    definitions:    Vec<usize>,
    /// The signature edit, once the target definition is seen.
    signature_edit: Option<Fix>,
    /// Zero-based position of the parameter among call arguments;
    /// `None` for a `self` receiver (call sites auto-ref).
    param_position: Option<usize>,
    /// One `&`-insertion per located call argument.
    call_edits:     Vec<Fix>,
    /// Bare-path references outside call position; any of them aborts.
    bare_path_refs: usize
}

impl Scan<'_> {
    /// Records the signature edit for a matching definition.
    fn record_definition(&mut self, signature: &syn::Signature) {
        self.definitions.push(self.source_index);
        for (position, input) in signature.inputs.iter().enumerate() {
            match input {
                syn::FnArg::Receiver(receiver)
                    if self.param == "self" && receiver.reference.is_none() =>
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

/// Builds a zero-width machine-applicable insertion, addressed to
/// `file` or to the finding's own file when `None`.
fn insertion(at: usize, text: &str, file: Option<PathBuf>) -> Fix {
    let offset = u32::try_from(at).unwrap_or(u32::MAX);
    Fix {
        file,
        rename_to: None,
        range: Range {
            start: offset,
            end:   offset
        },
        replacement: text.to_owned(),
        applicability: Applicability::MachineApplicable
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !self.collect_calls && self.owner.is_none() && item.sig.ident == self.bare {
            self.record_definition(&item.sig);
        }
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let name = match item.self_ty.as_ref() {
            syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
            _ => None
        };
        for entry in &item.items {
            if let syn::ImplItem::Fn(method) = entry
                && method.sig.ident == self.bare
                && !self.collect_calls
            {
                if item.trait_.is_none()
                    && self
                        .owner
                        .is_some_and(|owner| Some(owner.to_owned()) == name)
                {
                    self.record_definition(&method.sig);
                } else {
                    self.definitions.push(self.source_index);
                }
            }
        }
        syn::visit::visit_item_impl(self, item);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref()
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.bare)
        {
            if self.collect_calls {
                self.record_call(&call.args);
            }
            for argument in &call.args {
                self.visit_expr(argument);
            }
            return;
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.collect_calls && call.method == self.bare && self.owner.is_some() {
            self.record_call(&call.args);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if self.collect_calls
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.bare)
        {
            self.bare_path_refs += 1;
        }
        syn::visit::visit_expr_path(self, path);
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::borrow_fixes;
    use crate::source::Source;

    fn sources(text: &str) -> [Source; 1] {
        [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn private_function_gets_signature_and_caller_edits() {
        let text = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                    report.total }\nfn run(report: Report) -> u64 { summarize(report) }";
        let fixes = borrow_fixes(&sources(text), "summarize", "report", false);
        assert_eq!(fixes.len(), 2);
        assert!(fixes.iter().all(|fix| fix.replacement == "&"));
    }

    #[test]
    fn public_functions_get_no_fix() {
        let text = "struct Report { total: u64 }\npub fn summarize(report: Report) -> u64 { \
                    report.total }";
        assert!(borrow_fixes(&sources(text), "summarize", "report", true).is_empty());
    }

    #[test]
    fn function_pointer_use_aborts_the_fix() {
        let text = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                    report.total }\nfn pick() -> fn(Report) -> u64 { summarize }";
        assert!(borrow_fixes(&sources(text), "summarize", "report", false).is_empty());
    }

    #[test]
    fn duplicate_bare_names_abort_the_fix() {
        let text = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                    report.total }\nmod other { pub fn summarize(x: u64) -> u64 { x } }";
        assert!(borrow_fixes(&sources(text), "summarize", "report", false).is_empty());
    }

    #[test]
    fn owned_self_needs_only_the_signature_edit() {
        let text = "struct Report { total: u64 }\nimpl Report { fn summarize(self) -> u64 { \
                    self.total } }\nfn run(report: Report) -> u64 { report.summarize() }";
        let fixes = borrow_fixes(&sources(text), "Report::summarize", "self", false);
        assert_eq!(fixes.len(), 1);
    }

    #[test]
    fn already_borrowed_arguments_are_not_double_wrapped() {
        let text = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                    report.total }\nfn run(report: Report) -> u64 { summarize(report) }";
        let fixes = borrow_fixes(&sources(text), "summarize", "report", false);
        let signature_only = fixes.iter().filter(|fix| fix.range.start < 60).count();
        assert_eq!(signature_only, 1);
    }
}
