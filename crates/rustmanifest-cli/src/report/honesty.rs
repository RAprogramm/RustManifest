// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The coverage section: how much of the sources the engine could read,
//! stated plainly so the reader knows what the rest of the report is
//! worth.

use std::fmt::Write as _;

use rustmanifest_engine::Source;

/// How much of the sources the engine could read, printed last so the
/// reader learns what the rest of the report is worth.
pub(super) fn honesty(sources: &[Source]) -> String {
    let coverage = rustmanifest_engine::coverage(sources);
    let mut out = format!(
        "\n## coverage\n\n- files parsed: {}/{} ({:.1}%)\n",
        coverage.files_parsed,
        coverage.files_total,
        coverage.parse_rate()
    );
    if coverage.call_candidates_total == 0 {
        out.push_str("- calls seen: 0\n");
    } else {
        let _ = writeln!(
            out,
            "- calls resolved in-workspace: {}/{}, receivers left unknown: {}",
            coverage.call_candidates_resolved,
            coverage.call_candidates_total,
            coverage.call_candidates_unknown
        );
    }
    out.push_str("- unread sources are not a claim of absence\n");
    out
}
