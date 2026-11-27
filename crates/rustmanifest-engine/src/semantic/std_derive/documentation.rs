// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The library's own sentence about a property, quoted from where it is
//! written.
//!
//! Code proves what happens; the library's documentation states what it
//! promises. A finding that cites only the code leaves the reader to
//! take the analyzer's word for what the code means, so each property
//! names the file, the item and the identifying word that lead to the
//! sentence — and a sentence that cannot be found is reported as absent
//! rather than replaced by one written here.

use std::path::Path;

use super::fact::DocQuote;

/// Where each property's own documentation sentence lives in the
/// library, and the word that identifies the sentence inside it.
///
/// The library documents its own guarantees; a finding that cites the
/// code without the sentence beside it leaves the reader to take the
/// analyzer's word for what the code means. Both are quoted, and a
/// property whose sentence cannot be found carries none rather than a
/// substitute written here.
pub(super) const DOC_SOURCES: [(&str, &str, &str, &str); 4] = [
    (
        "panic-on-failure",
        "std/src/macros.rs",
        "println",
        "Panics if writing"
    ),
    ("format-machinery", "std/src/macros.rs", "println", "format"),
    (
        "lock-per-call:stdout",
        "std/src/io/stdio.rs",
        "Stdout",
        "lock"
    ),
    (
        "lock-per-call:stderr",
        "std/src/io/stdio.rs",
        "Stderr",
        "lock"
    )
];

/// Reads the library's own sentence for one property.
///
/// The item is located by name in the named file and its doc comment is
/// searched for the first sentence carrying the identifying word. A file
/// that does not parse, an item that is absent, or a doc comment that
/// never mentions the word all yield nothing.
pub(super) fn doc_quote(
    root: &Path,
    file: &str,
    item_name: &str,
    keyword: &str
) -> Option<DocQuote> {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed = syn::parse_file(&text).ok()?;
    let mut items = Vec::new();
    crate::ast::flattened(&parsed.items, &mut items);
    for item in items {
        let (attrs, name) = match item {
            syn::Item::Macro(declared) => (
                &declared.attrs,
                declared
                    .ident
                    .as_ref()
                    .map_or_default(ToString::to_string)
            ),
            syn::Item::Struct(declared) => (&declared.attrs, declared.ident.to_string()),
            syn::Item::Fn(declared) => (&declared.attrs, declared.sig.ident.to_string()),
            _ => continue
        };
        if name != item_name {
            continue;
        }
        if let Some(quote) = doc_sentence(attrs, keyword, &text, &path) {
            return Some(quote);
        }
    }
    None
}

/// First doc line carrying the identifying word, with its location.
fn doc_sentence(
    attrs: &[syn::Attribute],
    keyword: &str,
    text: &str,
    path: &Path
) -> Option<DocQuote> {
    for attribute in attrs {
        if !attribute.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(pair) = &attribute.meta else {
            continue;
        };
        let syn::Expr::Lit(literal) = &pair.value else {
            continue;
        };
        let syn::Lit::Str(line) = &literal.lit else {
            continue;
        };
        let sentence = line.value();
        let trimmed = sentence.trim();
        if trimmed.is_empty() || !trimmed.contains(keyword) {
            continue;
        }
        let located = text.find(trimmed).map_or(1, |offset| {
            text.get(..offset)
                .map_or(1, |before| before.matches('\n').count() + 1)
        });
        return Some(DocQuote {
            text: trimmed.to_owned(),
            file: path.to_path_buf(),
            line: located
        });
    }
    None
}
