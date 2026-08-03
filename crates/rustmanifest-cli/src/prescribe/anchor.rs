// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The prescription laid over the source it replaces, in the shape of a
//! compiler diagnostic.

mod site;

use std::fmt::Write as _;

use rustmanifest_engine::{OutputClaim, Source};

use self::site::{relative_file, span};

/// Renders the prescription against the source it replaces, in the
/// shape a compiler diagnostic uses: the offending line, a caret span
/// under the exact bytes, the label of what is wrong, then the
/// replacement and the note that justifies it.
pub(super) fn anchored(claim: &OutputClaim, sources: &[Source], prescription: &str) -> String {
    let Some((line_number, line_text, column, width)) = span(claim, sources) else {
        return format!("- `{}`\n\n```rust\n{prescription}\n```\n", claim.function);
    };
    let gutter = line_number.to_string().len();
    let pad = " ".repeat(gutter);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "warning: this write carries proven text through formatting"
    );
    let _ = writeln!(
        out,
        "{pad}--> {}:{line_number}:{}",
        relative_file(claim),
        column + 1
    );
    let _ = writeln!(out, "{pad} |");
    let _ = writeln!(out, "{line_number} | {line_text}");
    let _ = writeln!(
        out,
        "{pad} | {}{} the text is proven here, so the format machinery has nothing to resolve",
        " ".repeat(column),
        "^".repeat(width.max(1))
    );
    let _ = writeln!(out, "{pad} |");
    let _ = writeln!(
        out,
        "{pad} = help: write the bytes to the locked handle instead"
    );
    for replacement in prescription.lines() {
        let _ = writeln!(out, "{pad} |     {replacement}");
    }
    out.push('\n');
    out
}
