// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `method-naming` check: accessors are nouns, the `get_` prefix is
//! redundant (`STRUCTURE.md` §2).

use super::{Occurrence, flattened};

/// Reports functions and methods carrying a redundant `get_` prefix.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                report(&function.sig.ident, &mut found);
            }
            syn::Item::Impl(block) => {
                for member in &block.items {
                    if let syn::ImplItem::Fn(method) = member {
                        report(&method.sig.ident, &mut found);
                    }
                }
            }
            syn::Item::Trait(definition) => {
                for member in &definition.items {
                    if let syn::TraitItem::Fn(method) = member {
                        report(&method.sig.ident, &mut found);
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// Appends an occurrence when `ident` starts with `get_`.
fn report(ident: &syn::Ident, found: &mut Vec<Occurrence>) {
    let name = ident.to_string();
    if let Some(stripped) = name.strip_prefix("get_")
        && !stripped.is_empty()
    {
        found.push(Occurrence::new(
            &ident.span(),
            format!("method `{name}` uses a redundant `get_` prefix; call it `{stripped}`")
        ));
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
    use super::occurrences;

    fn count(source: &str) -> usize {
        occurrences(&syn::parse_file(source).unwrap()).len()
    }

    #[test]
    fn get_prefixed_functions_are_reported() {
        assert_eq!(count("fn get_name() {}"), 1);
        assert_eq!(
            count("struct D; impl D { fn get_value(&self) -> u8 { 0 } }"),
            1
        );
        assert_eq!(count("trait T { fn get_length(&self) -> usize; }"), 1);
    }

    #[test]
    fn noun_accessors_pass() {
        assert_eq!(
            count("struct D; impl D { fn name(&self) -> &str { \"\" } }"),
            0
        );
    }

    #[test]
    fn bare_get_passes() {
        assert_eq!(count("fn get_() {} fn get(index: usize) {}"), 0);
    }
}
