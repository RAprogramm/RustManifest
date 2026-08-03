// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The deviations section: every judgement against the canonical forms,
//! each followed by the evidence that carries it.

use std::fmt::Write as _;

use rustmanifest_engine::Source;

use super::{findings::analyze, location::relative};

/// Judgements against the canonical forms, each with its own evidence.
pub(super) fn judgement(sources: &[Source], root: &std::path::Path) -> String {
    let mut out = String::from("\n## deviations\n\n");
    let findings = analyze(sources);
    if findings.is_empty() {
        out.push_str("- no rule of the enabled pack applies\n");
        return out;
    }
    for finding in &findings {
        let _ = writeln!(
            out,
            "- {} `{}` — {}",
            format!("{:?}", finding.severity).to_lowercase(),
            finding.rule_id,
            finding.message
        );
        for step in &finding.trace {
            let _ = writeln!(out, "  - {} [{}]", step.fact, relative(&step.reason, root));
        }
    }
    out
}
