// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Documentation claims as facts: what an item's rustdoc *states*,
//! extracted structurally so truth rules can join it against what the
//! body provably does.
//!
//! Per function the extractor records whether docs exist, the leading
//! verb of the summary line, the declared `#`-sections (`Errors`,
//! `Panics`, `Safety`, `Examples`), and every backticked identifier the
//! prose mentions. Extraction is pure parsing — no interpretation.

use rustmanifest_schema::Range;

use super::expr::impl_type_name;
use crate::source::Source;

/// Structured documentation claims of one function or method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocFacts {
    /// Qualified name, matching the behavioral facts naming.
    pub function:     String,
    /// Index of the source the item was parsed from.
    pub source:       usize,
    /// Byte range of the item's identifier.
    pub range:        Range,
    /// Whether the item carries any doc comment.
    pub has_docs:     bool,
    /// Lowercased first word of the summary line, when docs exist.
    pub summary_verb: Option<String>,
    /// Lowercased names of declared `#`-sections in order of appearance.
    pub sections:     Vec<String>,
    /// Backticked identifier-shaped mentions, deduplicated and sorted.
    pub mentions:     Vec<String>
}

/// Extracts documentation claims for every function in `sources`; files
/// that do not parse as Rust contribute no facts.
#[must_use]
pub fn doc_facts(sources: &[Source]) -> Vec<DocFacts> {
    let mut facts = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    facts.push(item_docs(
                        index,
                        function.sig.ident.to_string(),
                        &function.sig.ident,
                        &function.attrs
                    ));
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        facts.push(item_docs(
                            index,
                            format!("{type_name}::{}", method.sig.ident),
                            &method.sig.ident,
                            &method.attrs
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    facts
}

/// Builds the doc claims of one item from its attributes.
fn item_docs(
    source: usize,
    function: String,
    ident: &syn::Ident,
    attrs: &[syn::Attribute]
) -> DocFacts {
    let lines = doc_lines(attrs);
    let summary_verb = lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .and_then(|line| line.split_whitespace().next())
        .map(|word| {
            word.trim_matches(|character: char| !character.is_ascii_alphabetic())
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty());
    let sections: Vec<String> = lines
        .iter()
        .filter_map(|line| line.trim().strip_prefix("# "))
        .map(|name| name.trim().to_ascii_lowercase())
        .collect();
    let mut mentions: Vec<String> = lines
        .iter()
        .flat_map(|line| {
            line.split('`')
                .skip(1)
                .step_by(2)
                .filter(|span| {
                    !span.is_empty()
                        && span
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '_')
                })
                .map(ToOwned::to_owned)
        })
        .collect();
    mentions.sort_unstable();
    mentions.dedup();
    let bytes = ident.span().byte_range();
    DocFacts {
        function,
        source,
        range: Range {
            start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
            end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
        },
        has_docs: !lines.is_empty(),
        summary_verb,
        sections,
        mentions
    }
}

/// Collects the string lines of every `#[doc = "..."]` attribute.
fn doc_lines(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            let syn::Meta::NameValue(pair) = &attr.meta else {
                return None;
            };
            let syn::Expr::Lit(literal) = &pair.value else {
                return None;
            };
            let syn::Lit::Str(text) = &literal.lit else {
                return None;
            };
            Some(text.value())
        })
        .collect()
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

    use super::doc_facts;
    use crate::source::Source;

    fn first(text: &str) -> super::DocFacts {
        doc_facts(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn summary_verb_sections_and_mentions_are_extracted() {
        let fact = first(
            "/// Returns the current `balance` of the `wallet`.\n///\n/// # Errors\n///\n/// \
             Fails when the ledger is closed.\n/// # Panics\n/// Never.\nfn read(wallet: u64) -> \
             u64 { wallet }"
        );
        assert!(fact.has_docs);
        assert_eq!(fact.summary_verb.as_deref(), Some("returns"));
        assert_eq!(fact.sections, vec!["errors", "panics"]);
        assert_eq!(fact.mentions, vec!["balance", "wallet"]);
    }

    #[test]
    fn undocumented_items_are_honest_blanks() {
        let fact = first("fn bare() {}");
        assert!(!fact.has_docs);
        assert!(fact.summary_verb.is_none());
        assert!(fact.sections.is_empty());
    }

    #[test]
    fn methods_are_qualified_like_behavior_facts() {
        let fact = first(
            "struct Wallet;\nimpl Wallet {\n    /// Opens the vault.\n    fn open(&self) {}\n}"
        );
        assert_eq!(fact.function, "Wallet::open");
        assert_eq!(fact.summary_verb.as_deref(), Some("opens"));
    }

    #[test]
    fn code_spans_with_paths_are_not_identifier_mentions() {
        let fact = first("/// Uses `std::fs::read` and `count`.\nfn go(count: u64) {}");
        assert_eq!(fact.mentions, vec!["count"]);
    }
}
