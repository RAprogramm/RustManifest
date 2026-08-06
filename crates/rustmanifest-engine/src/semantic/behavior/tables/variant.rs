// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a variant of an enum carries, where it carries exactly one value.
//!
//! A pattern taking that value apart binds a name to it, and the type of
//! that name is written in the declaration rather than at the position
//! reading it. Without this table such a name types to nothing, so every
//! field read and every method called on it refuses — which is most of
//! what an enum-walking body does.
//!
//! Only a variant with a single unnamed payload is recorded. Several
//! payloads make the type of one name a question of position that the
//! reader answers on its own, and a named payload is a field, already
//! answered by the field table.
//!
//! Each variant is noted twice — under its own name and under
//! `Enum.Variant` — because a pattern may write either. A bare name two
//! enums both declare resolves to neither: the scoped table refuses an
//! ambiguous key rather than picking one.

use syn::visit::Visit;

use super::super::scoped::Scoped;
use crate::source::Source;

/// Reads the single payload type of every variant that carries one.
pub(super) fn variant_payloads(sources: &[Source]) -> Scoped<String> {
    let mut table = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut harvest = Payloads {
            source: index,
            table:  &mut table
        };
        harvest.visit_file(file);
    }
    table
}

/// Walk collecting the payload types of one source.
struct Payloads<'table> {
    /// Source the declarations being read belong to.
    source: usize,
    /// Table under construction.
    table:  &'table mut Scoped<String>
}

impl<'ast> Visit<'ast> for Payloads<'_> {
    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        let owner = item.ident.to_string();
        for variant in &item.variants {
            let syn::Fields::Unnamed(unnamed) = &variant.fields else {
                continue;
            };
            let [only] = unnamed.unnamed.iter().collect::<Vec<_>>()[..] else {
                continue;
            };
            let Some(carried) = super::super::signature::type_last_segment(&only.ty) else {
                continue;
            };
            let named = variant.ident.to_string();
            self.table
                .note(format!("{owner}.{named}"), self.source, carried.clone());
            self.table.note(named, self.source, carried);
        }
        syn::visit::visit_item_enum(self, item);
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

    use super::variant_payloads;
    use crate::source::Source;

    fn source(text: &str) -> [Source; 1] {
        [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn a_single_payload_is_read_under_both_spellings() {
        let sources = source("enum Step { Named(Label), Empty }\nstruct Label;\n");
        let table = variant_payloads(&sources);
        assert_eq!(
            table.resolve("Step.Named", 0, &["own".to_owned()]).cloned(),
            Some("Label".to_owned())
        );
        assert_eq!(
            table.resolve("Named", 0, &["own".to_owned()]).cloned(),
            Some("Label".to_owned())
        );
        assert!(table.resolve("Empty", 0, &["own".to_owned()]).is_none());
    }

    #[test]
    fn several_payloads_are_left_alone() {
        let sources = source("enum Pair { Both(u8, u8) }\n");
        let table = variant_payloads(&sources);
        assert!(table.resolve("Both", 0, &["own".to_owned()]).is_none());
    }
}
