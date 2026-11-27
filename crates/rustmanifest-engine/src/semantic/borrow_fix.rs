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
//!
//! The evidence is gathered by [`scan`]; the edits it emits are shaped by
//! [`edit`]. This module owns only the gate and the two-phase drive.

mod edit;
mod scan;

use std::path::PathBuf;

use rustmanifest_schema::Fix;
use syn::visit::Visit;

use self::scan::Scan;
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
            let Some(file) = source.tree() else {
                continue;
            };
            scan.source_index = index;
            scan.current_path = source.path().to_path_buf();
            scan.visit_file(file);
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
        assert_eq!(
            borrow_fixes(&sources(text), "summarize", "report", true),
            [] as [rustmanifest_schema::Fix; 0]
        );
    }

    #[test]
    fn function_pointer_use_aborts_the_fix() {
        let text = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                    report.total }\nfn pick() -> fn(Report) -> u64 { summarize }";
        assert_eq!(
            borrow_fixes(&sources(text), "summarize", "report", false),
            [] as [rustmanifest_schema::Fix; 0]
        );
    }

    #[test]
    fn duplicate_bare_names_abort_the_fix() {
        let text = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                    report.total }\nmod other { pub fn summarize(x: u64) -> u64 { x } }";
        assert_eq!(
            borrow_fixes(&sources(text), "summarize", "report", false),
            [] as [rustmanifest_schema::Fix; 0]
        );
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
