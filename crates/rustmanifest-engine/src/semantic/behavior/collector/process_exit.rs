// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one site that produces a process status: the library's own
//! exit-status constants, its conversion from a number, and the
//! process-ending call.
//!
//! Every spelling is decided on the path it expands to through the file's
//! imports. A constant named `SUCCESS` on a type of the program's own is
//! not the library's exit status, and crediting it would put a status in
//! the report that the process never hands back.

use super::{
    FactCollector,
    catalog::{EXIT_CODE_TYPE, EXIT_FAILURE, EXIT_FROM, EXIT_SUCCESS, PROCESS_EXIT_PATH}
};
use crate::semantic::behavior::exit::{ExitRoute, ExitStatus, ProcessExit};

impl FactCollector {
    /// Records one site producing a process status, in body order.
    fn push_exit(&mut self, span: std::ops::Range<usize>, status: ExitStatus, route: ExitRoute) {
        self.facts.exits.push(ProcessExit {
            status,
            route,
            site: rustmanifest_schema::Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            },
            guard: self.site_guard()
        });
    }

    /// Records the status a call produces: the process-ending call, whose
    /// number is read where it is written as one, and the conversion of a
    /// number into the library's exit status.
    pub(super) fn record_exit_call(&mut self, call: &syn::ExprCall) {
        use syn::spanned::Spanned;
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return;
        };
        let segments = self.expanded_segments(&path.path);
        let span = call.span().byte_range();
        if segments == PROCESS_EXIT_PATH {
            let status = call.args.first().map_or(ExitStatus::Computed, code_of);
            self.push_exit(span, status, ExitRoute::Called);
            return;
        }
        if exit_code_member(&segments, EXIT_FROM) {
            let status = call.args.first().map_or(ExitStatus::Computed, code_of);
            self.push_exit(span, status, ExitRoute::Returned);
        }
    }

    /// Records the status a bare path produces, where it is one of the
    /// library's two named exit statuses.
    pub(super) fn record_exit_constant(&mut self, path: &syn::ExprPath) {
        use syn::spanned::Spanned;
        let segments = self.expanded_segments(&path.path);
        let status = if exit_code_member(&segments, EXIT_SUCCESS) {
            ExitStatus::Success
        } else if exit_code_member(&segments, EXIT_FAILURE) {
            ExitStatus::Failure
        } else {
            return;
        };
        self.push_exit(path.span().byte_range(), status, ExitRoute::Returned);
    }
}

/// Whether an expanded path names `member` on the library's exit-status
/// type. The type is matched on its own segment rather than on the whole
/// path, since the type is commonly imported by name.
fn exit_code_member(segments: &[String], member: &str) -> bool {
    let [.., owner, named] = segments else {
        return false;
    };
    owner == EXIT_CODE_TYPE && named == member
}

/// The status a written argument proves: a number written as a literal,
/// and nothing where the value is computed.
fn code_of(argument: &syn::Expr) -> ExitStatus {
    let syn::Expr::Lit(literal) = argument else {
        return ExitStatus::Computed;
    };
    let syn::Lit::Int(number) = &literal.lit else {
        return ExitStatus::Computed;
    };
    number
        .base10_parse::<u8>()
        .map_or(ExitStatus::Computed, |code| {
            if code == 0 {
                ExitStatus::Success
            } else {
                ExitStatus::Code(code)
            }
        })
}
