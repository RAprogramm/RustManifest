// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the parser accepted, and the constructs it left unexpanded.
//!
//! Parsing is where a source either enters the measurement or drops out of
//! it, and the same pass counts the `#[derive(...)]` attributes and
//! item-level macro invocations the accepted files carry. Both counts are
//! sightings and nothing more: no expansion happens anywhere in the
//! engine, so the items a derive or a macro would generate are invisible
//! to every other counter.

use syn::visit::Visit;

use crate::source::Source;

/// Parse outcome and macro sightings of one analyzed source slice.
pub(super) struct Sightings {
    /// Sources accepted by the Rust parser.
    pub(super) files_parsed: u64,
    /// `#[derive(...)]` attributes seen on items.
    pub(super) derives:      u64,
    /// Item-level macro invocations seen.
    pub(super) item_macros:  u64
}

/// Parses every source and counts the derive attributes and item-level
/// macro invocations of the files the parser accepts.
pub(super) fn sightings(sources: &[Source]) -> Sightings {
    let mut files_parsed = 0u64;
    let mut derives = 0u64;
    let mut item_macros = 0u64;
    for source in sources {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        files_parsed += 1;
        let mut counter = MacroCounter::default();
        counter.visit_file(&file);
        derives += counter.derives;
        item_macros += counter.item_macros;
    }
    Sightings {
        files_parsed,
        derives,
        item_macros
    }
}

/// Syntax visitor counting derive attributes and item-level macro
/// invocations of one parsed file.
#[derive(Default)]
struct MacroCounter {
    /// `#[derive(...)]` attributes seen so far.
    derives:     u64,
    /// Item-level macro invocations seen so far.
    item_macros: u64
}

impl<'ast> Visit<'ast> for MacroCounter {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if attribute.path().is_ident("derive") {
            self.derives += 1;
        }
        syn::visit::visit_attribute(self, attribute);
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        self.item_macros += 1;
        syn::visit::visit_item_macro(self, item);
    }
}
