// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The counts a prescription moves, and the library facts they rest on.

use std::fmt::Write as _;

use rustmanifest_engine::{Newline, OutputClaim, Source};

/// The counts the change moves, each with the fact behind it.
pub(super) fn derivation(claim: &OutputClaim, sources: &[Source]) -> String {
    let text = claim.write.payload.fully_known().unwrap_or_default();
    let bytes = text.len() + usize::from(claim.write.newline == Newline::Appended);
    let mut out = String::from("  before → after\n");
    let _ = writeln!(out, "  - format-argument constructions per write: 1 → 0");
    let _ = writeln!(out, "  - lock acquisitions per write: 1 → 1");
    let _ = writeln!(out, "  - bytes handed to the stream: {bytes} → {bytes}");
    let _ = writeln!(
        out,
        "  - a failed write: panic → returned error at `{}`",
        claim.function
    );
    for step in evidence(sources) {
        let _ = writeln!(out, "  - {step}");
    }
    out.push('\n');
    out
}

/// The library facts the counts rest on, quoted where the library states
/// them.
fn evidence(sources: &[Source]) -> Vec<String> {
    let _ = sources;
    rustmanifest_engine::library_output_facts()
        .into_iter()
        .map(|fact| format!("{} [{}:{}]", fact.snippet, fact.file.display(), fact.line))
        .collect()
}
