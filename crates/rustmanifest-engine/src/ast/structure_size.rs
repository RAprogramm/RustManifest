// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `structure-size` check: a structure has at most four fields; more
//! indicates mixed responsibilities (`STRUCTURE.md` §3).

use super::{Occurrence, flattened};

/// Maximum number of named fields a structure may declare.
const FIELD_LIMIT: usize = 4;

/// Reports structs declaring more than [`FIELD_LIMIT`] named fields.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        let syn::Item::Struct(definition) = item else {
            continue;
        };
        let syn::Fields::Named(fields) = &definition.fields else {
            continue;
        };
        let count = fields.named.len();
        if count > FIELD_LIMIT {
            found.push(Occurrence::new(
                &definition.ident.span(),
                format!(
                    "struct `{}` declares {count} fields (limit {FIELD_LIMIT}); split \
                     responsibilities through composition",
                    definition.ident
                )
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
    use super::occurrences;

    fn count(source: &str) -> usize {
        occurrences(&syn::parse_file(source).unwrap()).len()
    }

    #[test]
    fn oversized_struct_is_reported() {
        assert_eq!(
            count("struct User { a: u8, b: u8, c: u8, d: u8, e: u8 }"),
            1
        );
    }

    #[test]
    fn four_fields_pass() {
        assert_eq!(count("struct User { a: u8, b: u8, c: u8, d: u8 }"), 0);
    }

    #[test]
    fn tuple_and_unit_structs_pass() {
        assert_eq!(count("struct P(u8, u8, u8, u8, u8, u8); struct U;"), 0);
    }
}
