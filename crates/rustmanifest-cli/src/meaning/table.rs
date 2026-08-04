// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The reading form: one block per entry point, one aligned row per
//! write, and a single footnote for the properties of the standard
//! library rather than of the analyzed program.

use std::fmt::Write as _;

use rustmanifest_engine::{EntryMeaning, Multiplicity, OutputClaim};

use super::{
    completion::status_block,
    location::relative,
    wording::{certainty, entry_kind}
};

/// Renders the report as an aligned table with a single footnote.
pub(super) fn text_report(entries: &[EntryMeaning], root: &std::path::Path) -> String {
    if entries.is_empty() {
        return "no process entry point found — this source slice declares no program".to_owned();
    }
    let mut out = String::new();
    let mut standard_stream_seen = false;
    for entry in entries {
        let _ = writeln!(
            out,
            "{} ({})",
            relative(&entry.file, root),
            entry_kind(entry.kind)
        );
        if entry.claims.is_empty() {
            for sentence in entry.sentences() {
                let _ = writeln!(out, "  {sentence}");
            }
            out.push_str(&status_block(entry, root));
            out.push('\n');
            continue;
        }
        let rows: Vec<[String; 4]> = entry.claims.iter().map(row).collect();
        let widths = column_widths(&rows);
        for (row, claim) in rows.iter().zip(&entry.claims) {
            standard_stream_seen = true;
            let _ = writeln!(
                out,
                "  {:<certainty$}  {:<stream$}  {:<payload$}  {}",
                row[0],
                row[1],
                row[2],
                row[3],
                certainty = widths[0],
                stream = widths[1],
                payload = widths[2]
            );
            if let Some(reason) = claim.weakening() {
                let _ = writeln!(
                    out,
                    "  {:<width$}  weakened by {reason}",
                    "",
                    width = widths[0]
                );
            }
        }
        out.push_str(&status_block(entry, root));
        out.push('\n');
    }
    if standard_stream_seen {
        out.push_str(
            "Writes to a standard stream are attempts: the library can divert them through its \
             capture hook, and a failed write panics instead of writing.\n"
        );
    }
    out.trim_end().to_owned()
}

/// The four columns of one row: certainty, stream, text, function.
fn row(claim: &OutputClaim) -> [String; 4] {
    let repetition = if claim.write.multiplicity == Multiplicity::Unbounded {
        " (per iteration)"
    } else {
        ""
    };
    [
        certainty(claim.execution).to_owned(),
        claim.write.stream.to_string(),
        format!("{}{repetition}", claim.payload_text()),
        claim.function.clone()
    ]
}

/// Width of every column but the last.
fn column_widths(rows: &[[String; 4]]) -> [usize; 3] {
    let mut widths = [0usize; 3];
    for row in rows {
        for (index, width) in widths.iter_mut().enumerate() {
            *width = (*width).max(row[index].chars().count());
        }
    }
    widths
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::Path;

    use rustmanifest_engine::{Completion, EntryKind, EntryMeaning, OUTPUT_SINK_SET};

    use super::text_report;

    fn silent_entry() -> EntryMeaning {
        EntryMeaning {
            file:       "src/main.rs".to_owned(),
            kind:       EntryKind::Plain,
            claims:     Vec::new(),
            completion: Completion::Nothing,
            exits:      Vec::new()
        }
    }

    /// A program that writes nothing still ends somehow, and the rule its
    /// signature fixes is stated whether or not any site was recorded.
    #[test]
    fn the_status_rule_is_stated_even_where_no_site_produces_one() {
        let report = text_report(&[silent_entry()], Path::new("."));
        assert!(report.contains("status:"), "{report}");
    }

    #[test]
    fn an_entry_with_no_claim_names_the_recognizer_set_that_found_none() {
        let report = text_report(&[silent_entry()], Path::new("."));
        assert!(
            report.contains(&format!("recognizer set v{OUTPUT_SINK_SET}")),
            "negative result must state its recognizer set: {report}"
        );
    }
}
