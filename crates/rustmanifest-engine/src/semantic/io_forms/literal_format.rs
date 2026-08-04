// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The fully proven literal carried through the formatting machinery,
//! where a direct byte write skips it.

use super::{
    catalogue::{CONSTANT_TEXT, library_knowledge, support},
    finding::{detail, memory_steps, trace},
    shipped_code::shipped
};
use crate::semantic::{
    behavior::{Conduit, Payload},
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
            found.push(Insight::new(
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
            ));
        }
    }
    found
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
    use crate::{semantic::workspace::Workspace, source::Source};

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    #[test]
    fn a_constant_through_the_format_machinery_is_reported() {
        let constant = literal_format_insights(&Workspace::new(&sources(
            "fn main() {\n    println!(\"hi\");\n}"
        )));
        assert_eq!(constant.len(), 1);

        let computed = literal_format_insights(&Workspace::new(&sources(
            "fn main() {\n    println!(\"{name}\");\n}"
        )));
        assert!(computed.is_empty());
    }

    /// The form this check prescribes is not a deviation from itself: a
    /// literal written as bytes to the locked handle enters no formatting
    /// machinery, so there is no argument pack to charge it for.
    #[test]
    fn the_prescribed_byte_write_is_not_reported_against_itself() {
        let prescribed = literal_format_insights(&Workspace::new(&sources(
            "fn main() -> std::io::Result<()> {\n    \
             std::io::stdout().lock().write_all(b\"hi\")\n}"
        )));
        assert!(prescribed.is_empty());

        let bound = literal_format_insights(&Workspace::new(&sources(
            "fn main() -> std::io::Result<()> {\n    let mut out = std::io::stdout().lock();\n    \
             out.write_all(b\"hi\")\n}"
        )));
        assert!(bound.is_empty());
    }
}
