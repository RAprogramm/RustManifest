// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The fully proven literal carried through the formatting machinery,
//! where a direct byte write skips it.

use super::{
    catalogue::{CONSTANT_TEXT, library_knowledge, support},
    finding::{detail, memory_steps, trace},
    shipped_code::shipped
};
use crate::{
    semantic::{
        behavior::{Payload, behavior},
        insight::Insight
    },
    source::Source
};

/// Reports a fully proven literal carried through the formatting
/// machinery, where a direct byte write skips it.
pub(in crate::semantic) fn literal_format_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if !shipped(sources, &fact) {
            continue;
        }
        for write in &fact.output_writes {
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
    use crate::source::Source;

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    #[test]
    fn a_constant_through_the_format_machinery_is_reported() {
        let constant = literal_format_insights(&sources("fn main() {\n    println!(\"hi\");\n}"));
        assert_eq!(constant.len(), 1);

        let computed =
            literal_format_insights(&sources("fn main() {\n    println!(\"{name}\");\n}"));
        assert!(computed.is_empty());
    }
}
