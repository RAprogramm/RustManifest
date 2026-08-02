// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Analysis of macro-expanded source: what derives and macros generate.
//!
//! The engine's normal tiers parse surface syntax, so items produced by
//! `#[derive(...)]` or item-level macros stay invisible; coverage reports
//! them only as sightings. This module closes that gap for callers that
//! obtain expanded source (for example via
//! `cargo +nightly rustc -- -Zunpretty=expanded`): [`expansion`] parses the
//! expanded text and measures what the expansion made visible — generated
//! `impl` blocks and the traits they implement.
//!
//! Expanded output collapses a whole crate into one stream with compiler
//! line numbers, so it is suitable for measurement, not for anchoring
//! findings to user files.

use std::collections::BTreeMap;

use syn::visit::Visit;

/// What one expanded crate made visible to the parser.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpansionReport {
    /// Items of any kind in the expanded stream, nested modules included.
    pub items_expanded: u64,
    /// `impl` blocks in the expanded stream.
    pub impls_total:    u64,
    /// `impl` blocks carrying `#[automatically_derived]`, i.e. produced by
    /// a derive expansion.
    pub derived_impls:  u64,
    /// Derived trait name to the number of `impl` blocks implementing it.
    pub derived_traits: BTreeMap<String, u64>
}

impl ExpansionReport {
    /// Folds another crate's report into this one, summing every counter.
    pub fn absorb(&mut self, other: &Self) {
        self.items_expanded += other.items_expanded;
        self.impls_total += other.impls_total;
        self.derived_impls += other.derived_impls;
        for (name, count) in &other.derived_traits {
            *self.derived_traits.entry(name.clone()).or_default() += count;
        }
    }
}

/// Measures what macro expansion made visible in `expanded` source text.
///
/// Returns [`None`] when the expanded text does not parse as a Rust file;
/// callers report such crates as skipped rather than guessing.
#[must_use]
pub fn expansion(expanded: &str) -> Option<ExpansionReport> {
    let file = syn::parse_file(expanded).ok()?;
    let mut counter = ExpansionCounter::default();
    counter.visit_file(&file);
    Some(counter.report)
}

/// Syntax visitor accumulating the [`ExpansionReport`] counters.
#[derive(Default)]
struct ExpansionCounter {
    /// Counters accumulated so far.
    report: ExpansionReport
}

impl<'ast> Visit<'ast> for ExpansionCounter {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        self.report.items_expanded += 1;
        syn::visit::visit_item(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        self.report.impls_total += 1;
        let derived = item
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("automatically_derived"));
        if derived {
            self.report.derived_impls += 1;
            if let Some((path, _)) = &item.trait_
                && let Some(segment) = path.segments.last()
            {
                *self
                    .report
                    .derived_traits
                    .entry(segment.ident.to_string())
                    .or_default() += 1;
            }
        }
        syn::visit::visit_item_impl(self, item);
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
    use super::expansion;

    const EXPANDED: &str = "pub struct User { pub id: u64 }\n#[automatically_derived]\nimpl \
                            ::core::fmt::Debug for User { }\n#[automatically_derived]\nimpl \
                            ::core::clone::Clone for User { }\nimpl User { pub fn id(&self) -> \
                            u64 { self.id } }\nmod inner { pub struct Inner; \
                            #[automatically_derived] impl ::core::default::Default for Inner { } \
                            }";

    #[test]
    fn derived_impls_are_separated_from_hand_written() {
        let report = expansion(EXPANDED).unwrap();
        assert_eq!(report.impls_total, 4);
        assert_eq!(report.derived_impls, 3);
    }

    #[test]
    fn derived_traits_are_counted_by_last_path_segment() {
        let report = expansion(EXPANDED).unwrap();
        assert_eq!(report.derived_traits["Debug"], 1);
        assert_eq!(report.derived_traits["Clone"], 1);
        assert_eq!(report.derived_traits["Default"], 1);
    }

    #[test]
    fn nested_module_items_are_counted() {
        let report = expansion(EXPANDED).unwrap();
        assert_eq!(report.items_expanded, 7);
    }

    #[test]
    fn unparseable_stream_yields_none() {
        assert!(expansion("fn broken(").is_none());
    }

    #[test]
    fn absorb_sums_counters_and_merges_traits() {
        let mut total = expansion(EXPANDED).unwrap();
        total.absorb(&expansion(EXPANDED).unwrap());
        assert_eq!(total.derived_impls, 6);
        assert_eq!(total.derived_traits["Debug"], 2);
    }
}
