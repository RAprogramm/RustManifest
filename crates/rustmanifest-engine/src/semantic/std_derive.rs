// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge derived from the standard library's own sources.
//!
//! The library ships its source with the toolchain, and it is ordinary
//! code in the same language the engine already reads. Everything the
//! output forms rest on is therefore a fact with a location, not an
//! assertion someone typed into this repository: which internal function
//! a print macro calls, whether that function takes the stream's lock,
//! whether it can panic, and which writer chain each stream's handle
//! declares.
//!
//! Every fact carries the library's own text at the range it was read
//! from, so a consumer quotes the evidence instead of paraphrasing it.
//! The sentence around the quote is this engine's wording, but every
//! name in it — the function, the method, the writer types — was read
//! from the source, and the quote is what proves it.
//!
//! Properties that differ between the two standard streams are named per
//! stream, `property:stdout` and `property:stderr`, because the library
//! gives the two streams different handles with different writer chains
//! and an independent lock each. A property derived from one stream's
//! declaration says nothing about the other, and an unqualified name
//! would hide exactly that.
//!
//! A derivation that fails to find its evidence yields nothing. The
//! caller then has no form to judge against and must stay silent, which
//! is the honest outcome — the alternative is exactly the hand-written
//! constant this module exists to remove.

mod buffering;
mod capacity;
mod citation;
mod documentation;
mod evidence;
mod expression;
mod fact;
mod toolchain;

use std::path::Path;

use self::{
    capacity::buffer_capacity,
    documentation::{DOC_SOURCES, doc_quote},
    evidence::Evidence,
    expression::ExpressionKind
};
pub use self::{
    fact::{StdFact, StdOutputKnowledge},
    toolchain::std_source_root
};
use super::behavior::behavior;
use crate::source::Source;

/// The two standard streams: the handle type the library declares for
/// each one, and the name a per-stream property carries.
const HANDLES: [(&str, &str); 2] = [("Stdout", "stdout"), ("Stderr", "stderr")];

/// Internal function every print macro funnels through, generic over the
/// stream it is handed, which is why the properties read out of it hold
/// for both streams and carry no stream in their name.
const PRINT_FUNCTION: &str = "print_to";

/// Derives what the print path guarantees, by reading the library's own
/// standard-output module.
///
/// Per stream: that the handle takes its own lock inside the call that
/// writes through it, and which writer chain the handle declares. Shared
/// between the streams: that the print path routes its argument through
/// the formatting machinery, and that it panics when the write fails —
/// both read out of the one generic function both streams reach.
#[must_use]
pub fn output_knowledge(root: &Path) -> StdOutputKnowledge {
    let stdio = root.join("std/src/io/stdio.rs");
    let Ok(text) = std::fs::read_to_string(&stdio) else {
        return StdOutputKnowledge::default();
    };
    let Ok(parsed) = syn::parse_file(&text) else {
        return StdOutputKnowledge::default();
    };
    let mut items = Vec::new();
    crate::ast::flattened(&parsed.items, &mut items);
    let sources = [Source::new(stdio.clone(), text.clone())];
    let behavior_facts = behavior(&sources);
    let library = Evidence {
        file:  &stdio,
        text:  &text,
        items: &items
    };
    let mut derived = Vec::new();
    for (handle, stream) in HANDLES {
        let writes = format!("{handle}::write_fmt");
        if behavior_facts.iter().any(|fact| fact.function == writes)
            && let Some(range) =
                library.method_expression(handle, "write_fmt", ExpressionKind::LockCall)
        {
            derived.push(library.fact(
                &format!("lock-per-call:{stream}"),
                &format!("`{writes}` takes the {stream} lock inside the call it serves"),
                range
            ));
        }
        if let Some(fact) = library.buffering(handle, stream) {
            derived.push(fact);
        }
    }
    if behavior_facts
        .iter()
        .any(|fact| fact.function == PRINT_FUNCTION)
    {
        if let Some(range) = library.free_expression(PRINT_FUNCTION, ExpressionKind::FormatWrite) {
            derived.push(library.fact(
                "format-machinery",
                &format!("`{PRINT_FUNCTION}` writes through the formatting machinery"),
                range
            ));
        }
        if let Some(range) = library.free_expression(PRINT_FUNCTION, ExpressionKind::Panic) {
            derived.push(library.fact(
                "panic-on-failure",
                &format!("`{PRINT_FUNCTION}` panics when the write fails"),
                range
            ));
        }
    }
    if let Some(fact) = buffer_capacity(root) {
        derived.push(fact);
    }
    for fact in &mut derived {
        if let Some((_, file, item, keyword)) = DOC_SOURCES
            .iter()
            .find(|(property, _, _, _)| *property == fact.property)
        {
            fact.doc = doc_quote(root, file, item, keyword);
        }
    }
    StdOutputKnowledge {
        facts: derived
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
    use super::{citation::quoted, output_knowledge, std_source_root};

    #[test]
    fn the_print_path_properties_come_from_the_library_itself() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        for property in [
            "lock-per-call:stdout",
            "lock-per-call:stderr",
            "format-machinery",
            "panic-on-failure",
            "line-buffered:stdout",
            "unbuffered:stderr"
        ] {
            let fact = knowledge
                .fact(property)
                .unwrap_or_else(|| panic!("`{property}` must be derived from the library source"));
            assert!(fact.range.end > fact.range.start);
            assert!(fact.file.ends_with("std/src/io/stdio.rs"));
            assert!(!fact.observed.is_empty());
        }
    }

    /// Each stream's buffering must be derived from that stream's own
    /// declaration and registered under that stream's name, so a form can
    /// never carry one stream's evidence to the other.
    #[test]
    fn buffering_is_derived_per_stream_and_never_shared() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        assert!(!knowledge.has("line-buffered"));
        assert!(!knowledge.has("lock-per-call"));
        assert!(!knowledge.has("line-buffered:stderr"));
        assert!(!knowledge.has("unbuffered:stdout"));

        let stdout = knowledge.fact("line-buffered:stdout").unwrap();
        assert!(stdout.snippet.contains("LineWriter"));
        let stderr = knowledge.fact("unbuffered:stderr").unwrap();
        assert!(stderr.snippet.contains("StderrRaw"));
        assert!(!stderr.snippet.contains("LineWriter"));
        assert_ne!(stdout.range, stderr.range);
    }

    /// Every fact quotes the library's own text at the range it names, so
    /// a trace step shows the source and not a sentence from this
    /// repository.
    #[test]
    fn every_fact_quotes_the_source_text_at_its_own_range() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        assert!(!knowledge.facts.is_empty());
        for fact in &knowledge.facts {
            let text = std::fs::read_to_string(&fact.file).unwrap();
            let quoted = quoted(&text, fact.range);
            assert_eq!(fact.snippet, quoted);
            assert!(!fact.snippet.is_empty());
            assert!(!fact.snippet.contains('\n'));
            let line = text
                .lines()
                .nth(fact.line - 1)
                .expect("the recorded line exists in the file");
            let head = fact
                .snippet
                .split(' ')
                .next()
                .expect("a quote has a first token");
            assert!(
                line.contains(head),
                "`{}` must start on line {}: {line}",
                fact.snippet,
                fact.line
            );
        }
    }

    /// The two lock facts must come from two different places: one per
    /// handle, because the library gives each stream its own lock.
    #[test]
    fn each_stream_proves_its_own_lock() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        let stdout = knowledge.fact("lock-per-call:stdout").unwrap();
        let stderr = knowledge.fact("lock-per-call:stderr").unwrap();
        assert_ne!(stdout.range, stderr.range);
    }
}
