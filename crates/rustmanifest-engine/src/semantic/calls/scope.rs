// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The written module each item stands in, read from the inline module
//! spans of every source — the granularity an unqualified call needs.
//!
//! A file is not a scope. A file may hold a top-level function and a
//! test module declaring the same name, and an unqualified call means
//! whichever of them stands in the calling scope. Reading the innermost
//! inline module containing a byte answers that exactly, and makes the
//! glob question moot: an item declared in the calling scope outranks
//! every globbed name by Rust's own resolution, so a scope that
//! declares the name settles the call whatever the file imports.

use rustmanifest_schema::Range;

use crate::source::Source;

/// The inline module spans of every source, innermost-resolvable.
#[derive(Debug, Default)]
pub struct Scopes {
    /// Byte spans of inline modules, per source index.
    spans: Vec<Vec<Range>>
}

impl Scopes {
    /// Reads the inline module spans of every source.
    #[must_use]
    pub fn of(sources: &[Source]) -> Self {
        Self {
            spans: sources
                .iter()
                .map(|source| {
                    let mut found = Vec::new();
                    if let Some(file) = source.tree() {
                        collect(&file.items, &mut found);
                    }
                    found
                })
                .collect()
        }
    }

    /// Whether two byte positions of one source stand in the same
    /// innermost inline module — the file's top level counting as one
    /// scope of its own.
    #[must_use]
    pub fn shared(&self, source: usize, one: u32, other: u32) -> bool {
        let Some(spans) = self.spans.get(source) else {
            return true;
        };
        innermost(spans, one) == innermost(spans, other)
    }
}

/// The innermost span containing `at`, identified by its bounds.
fn innermost(spans: &[Range], at: u32) -> Option<(u32, u32)> {
    spans
        .iter()
        .filter(|span| span.start <= at && at < span.end)
        .min_by_key(|span| span.end - span.start)
        .map(|span| (span.start, span.end))
}

/// Collects the spans of every inline module, at any depth.
fn collect(items: &[syn::Item], into: &mut Vec<Range>) {
    for item in items {
        if let syn::Item::Mod(module) = item
            && let Some((_, inner)) = &module.content
        {
            let bytes = syn::spanned::Spanned::span(module).byte_range();
            into.push(Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            });
            collect(inner, into);
        }
    }
}
