// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The fully proven literal carried through the formatting machinery,
//! where a direct byte write skips it.

use rustmanifest_schema::{Applicability, Fix};

use super::{
    catalogue::{CONSTANT_TEXT, library_knowledge, support},
    finding::{detail, memory_steps, trace},
    shipped_code::shipped
};
use crate::semantic::{
    behavior::{Buffering, Conduit, Newline, OutputWrite, Payload, Stream},
    insight::Insight,
    workspace::Workspace
};

/// Reports a fully proven literal carried through the formatting
/// machinery, where a direct byte write skips it.
///
/// A write that already puts its bytes on the stream directly is the form
/// this check prescribes, not a deviation from it: the cost counted here
/// — one argument pack built and walked by a formatter — is a cost of the
/// formatting machinery, and a site that never entered it pays nothing.
/// Such a site is passed over in silence rather than charged for
/// machinery it does not use.
pub(in crate::semantic) fn literal_format_insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let library = library_knowledge();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        if !shipped(sources, fact) {
            continue;
        }
        for write in &fact.output_writes {
            if write.conduit != Conduit::Formatting {
                continue;
            }
            let Some(derived) = support(&CONSTANT_TEXT, write.stream, library) else {
                continue;
            };
            let Payload::Literal(text) = &write.payload else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            found.push(
                Insight::new(
                    fact.source,
                    write.site,
                    detail(
                        &CONSTANT_TEXT,
                        &fact.function,
                        &format!("the written text is fully proven: `{text}`"),
                        1
                    ),
                    {
                        let mut steps = trace(
                            &format!("the written text is fully proven: `{text}`"),
                            "payload recorded as a literal with no unproven positions",
                            derived
                        );
                        steps.extend(memory_steps(text, write, library));
                        steps
                    }
                )
                .with_fixes(byte_write_fix(text, write))
            );
        }
    }
    found
}

/// The prescribed form as an edit: the proven text written to the
/// locked handle directly, terminator kept, failure still ending the
/// path as the macro would.
///
/// Marked as needing review rather than machine-applicable: the byte
/// write is semantically faithful, but the panic message changes and a
/// surrounding style may prefer a bound handle — judgments the facts do
/// not carry.
fn byte_write_fix(text: &str, write: &OutputWrite) -> Vec<Fix> {
    if write.buffering == Buffering::Block {
        return Vec::new();
    }
    let handle = match write.stream {
        Stream::Stdout => "stdout",
        Stream::Stderr => "stderr",
        _ => return Vec::new()
    };
    let mut written = text.to_owned();
    if write.newline == Newline::Appended {
        written.push('\n');
    }
    vec![Fix {
        file:          None,
        rename_to:     None,
        range:         write.site,
        replacement:   format!(
            "std::io::Write::write_all(&mut std::io::{handle}().lock(), \
             {written:?}.as_bytes()).expect(\"write to {handle}\")"
        ),
        applicability: Applicability::MaybeIncorrect
    }]
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

    use super::literal_format_insights;
    use crate::source::Source;

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    /// The insights of a fixture read against what a compiler settles
    /// about it: the standard macros it writes and the types it
    /// declares, without which the readings above answer nothing.
    fn insights_over(sources: &[Source]) -> Vec<crate::semantic::insight::Insight> {
        let settled = crate::semantic::settling::settled_over(sources);
        literal_format_insights(&crate::semantic::workspace::Workspace::resolved(sources, &settled))
    }

    #[test]
    fn a_constant_through_the_format_machinery_is_reported() {
        let constant = insights_over(&sources(
            "fn main() {\n    println!(\"hi\");\n}"
        ));
        assert_eq!(constant.len(), 1);

        let computed = insights_over(&sources(
            "fn main() {\n    println!(\"{name}\");\n}"
        ));
        assert!(computed.is_empty());
    }

    /// The finding carries the prescribed form as an edit: the proven
    /// text written to the locked handle, terminator kept, and the
    /// escaping of the written literal preserved.
    #[test]
    fn the_finding_carries_the_prescribed_byte_write_as_a_fix() {
        let text = "fn main() {\n    println!(\"hi \\\"there\\\"\");\n}";
        let constant = insights_over(&sources(text));
        let fixes = &constant[0].fixes;
        assert_eq!(fixes.len(), 1);
        assert_eq!(
            fixes[0].replacement,
            "std::io::Write::write_all(&mut std::io::stdout().lock(), \"hi \\\"there\\\"\\n\"\
             .as_bytes()).expect(\"write to stdout\")"
        );
        let replaced = &text[fixes[0].range.start as usize..fixes[0].range.end as usize];
        assert_eq!(replaced, "println!(\"hi \\\"there\\\"\")");
    }

    /// A write into a caller-supplied stream names no handle to lock,
    /// so no fix is synthesized for it.
    #[test]
    fn a_caller_supplied_stream_gets_no_fix() {
        let held = insights_over(&sources(
            "fn tell(out: &mut impl std::io::Write) {\n    writeln!(out, \"hi\").ok();\n}"
        ));
        assert!(held.iter().all(|insight| insight.fixes.is_empty()));
    }

    /// The form this check prescribes is not a deviation from itself: a
    /// literal written as bytes to the locked handle enters no formatting
    /// machinery, so there is no argument pack to charge it for.
    #[test]
    fn the_prescribed_byte_write_is_not_reported_against_itself() {
        let prescribed = insights_over(&sources(
            "fn main() -> std::io::Result<()> {\n    \
             std::io::stdout().lock().write_all(b\"hi\")\n}"
        ));
        assert!(prescribed.is_empty());

        let bound = insights_over(&sources(
            "fn main() -> std::io::Result<()> {\n    let mut out = std::io::stdout().lock();\n    \
             out.write_all(b\"hi\")\n}"
        ));
        assert!(bound.is_empty());
    }
}
