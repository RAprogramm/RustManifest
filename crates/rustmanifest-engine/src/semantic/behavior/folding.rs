// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Closing the holes a named string constant already answers.
//!
//! A format position naming a constant is not unknown: the constant's
//! value is written in the source, and reading it is a lookup rather than
//! an evaluation. Closing such a position turns a template back into the
//! text the program actually writes.
//!
//! Three conditions, all required, none of them guessable:
//!
//! - the name resolves to a constant whose value is a plain string literal, in
//!   this file or unambiguously in this crate;
//! - the position asks for the value as it stands — a format specification of
//!   any kind (a width, a precision, the debug spelling) would render something
//!   other than the literal, so the position stays open;
//! - nothing in the function binds that name. A parameter, a `let`, a closure
//!   argument or a pattern binding of the same name shadows the constant, and
//!   which one a position reads is then a question of scopes this tier does not
//!   resolve.

use std::collections::BTreeSet;

use syn::visit::Visit;

use super::{
    output::{HoleSource, Payload, Segment},
    scoped::Scoped,
    types::FunctionFacts
};

/// Closes every write's format positions that a named string constant
/// already answers, leaving every other position open.
pub(super) fn fold_constants(
    facts: &mut FunctionFacts,
    block: &syn::Block,
    constants: &Scoped<String>,
    source: usize,
    crates: &[String]
) {
    if facts.output_writes.is_empty() {
        return;
    }
    let mut bound = BoundNames {
        names: BTreeSet::new()
    };
    bound.visit_block(block);
    for write in &mut facts.output_writes {
        let Payload::Template(segments) = &write.payload else {
            continue;
        };
        let closed: Vec<Segment> = segments
            .iter()
            .map(|segment| {
                proven(segment, constants, source, crates, &bound.names)
                    .map_or_else(|| segment.clone(), Segment::Text)
            })
            .collect();
        write.payload = settled(closed);
    }
}

/// The text a segment is proven to render, where it is a bare position
/// naming an unshadowed string constant.
fn proven(
    segment: &Segment,
    constants: &Scoped<String>,
    source: usize,
    crates: &[String],
    bound: &BTreeSet<String>
) -> Option<String> {
    let Segment::Hole(hole) = segment else {
        return None;
    };
    if !hole.spec.is_empty() {
        return None;
    }
    let HoleSource::Named(name) = &hole.source else {
        return None;
    };
    if bound.contains(name) {
        return None;
    }
    constants.resolve(name, source, crates).cloned()
}

/// The payload a closed segment list amounts to: one proven text where no
/// position is left open, and the template itself otherwise.
fn settled(segments: Vec<Segment>) -> Payload {
    if segments
        .iter()
        .any(|segment| matches!(segment, Segment::Hole(_)))
    {
        return Payload::Template(segments);
    }
    Payload::Literal(
        segments
            .iter()
            .map(|segment| match segment {
                Segment::Text(text) => text.as_str(),
                Segment::Hole(_) => ""
            })
            .collect()
    )
}

/// Walk collecting every name a function body binds, so a constant of the
/// same name is never read in its place.
struct BoundNames {
    /// Names bound anywhere in the body.
    names: BTreeSet<String>
}

impl<'ast> Visit<'ast> for BoundNames {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.names.insert(pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }
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

    use super::super::behavior;
    use crate::source::Source;

    fn written(text: &str) -> Option<String> {
        let sources = [Source::new(PathBuf::from("src/main.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert_eq!(main.output_writes.len(), 1);
        main.output_writes[0]
            .payload
            .fully_known()
            .map(ToOwned::to_owned)
    }

    /// A constant's value is written in the source, so a position naming
    /// it is answered rather than left open.
    #[test]
    fn a_position_naming_a_string_constant_is_closed() {
        assert_eq!(
            written(
                "const GREETING: &str = \"Hello, world!\";\nfn main() { println!(\"{GREETING}\"); }"
            ),
            Some("Hello, world!".to_owned())
        );
        assert_eq!(
            written(
                "static GREETING: &str = \"Hello\";\nfn main() { println!(\"{GREETING}, world!\"); }"
            ),
            Some("Hello, world!".to_owned())
        );
    }

    /// What the position renders must be the literal itself. A format
    /// specification renders something else, and a name the body binds may
    /// not be the constant at all.
    #[test]
    fn a_position_answered_by_anything_but_the_literal_stays_open() {
        let debug_spelling = "const GREETING: &str = \"Hello, world!\";\nfn main() { \
                              println!(\"{GREETING"
            .to_owned()
            + ":?}\"); }";
        assert_eq!(written(&debug_spelling), None);
        assert_eq!(
            written(
                "const GREETING: &str = \"Hello, world!\";\nfn main() { let GREETING = other(); \
                 println!(\"{GREETING}\"); }"
            ),
            None
        );
        assert_eq!(
            written("const LENGTH: usize = 13;\nfn main() { println!(\"{LENGTH}\"); }"),
            None
        );
    }
}
