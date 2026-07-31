// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `immutability-first` check: prefer consuming builders and returned values
//! over `&mut self` setters (`STRUCTURE.md` §7).
//!
//! A method fires only when both conditions hold: its name starts with
//! `set_` and its receiver is `&mut self`. Consuming builders
//! (`fn name(mut self, ..) -> Self`) never fire.

use super::{Occurrence, flattened};

/// Reports `set_*` methods taking `&mut self` in impl blocks and trait
/// declarations.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        match item {
            syn::Item::Impl(block) => {
                for member in &block.items {
                    if let syn::ImplItem::Fn(method) = member {
                        report(&method.sig, &mut found);
                    }
                }
            }
            syn::Item::Trait(definition) => {
                for member in &definition.items {
                    if let syn::TraitItem::Fn(method) = member {
                        report(&method.sig, &mut found);
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// Appends an occurrence when `sig` names a `set_*` method taking `&mut
/// self`.
fn report(sig: &syn::Signature, found: &mut Vec<Occurrence>) {
    let name = sig.ident.to_string();
    let Some(stripped) = name.strip_prefix("set_") else {
        return;
    };
    if stripped.is_empty() {
        return;
    }
    let Some(receiver) = sig.receiver() else {
        return;
    };
    if receiver.reference.is_none() || receiver.mutability.is_none() {
        return;
    }
    found.push(Occurrence::new(
        &sig.ident.span(),
        format!(
            "method `{name}` mutates through `&mut self`; prefer a consuming `fn \
             {stripped}(mut self, ..) -> Self` or returning a new value"
        ),
    ));
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
    fn mut_ref_setter_is_reported() {
        let source = "struct R { b: u8 } impl R { fn set_b(&mut self, b: u8) { self.b = b; } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn trait_declared_setter_is_reported() {
        assert_eq!(
            count("trait T { fn set_name(&mut self, name: String); }"),
            1
        );
    }

    #[test]
    fn consuming_setter_passes() {
        let source = "struct R { b: u8 } impl R { fn set_b(mut self, b: u8) -> Self { self.b = \
                      b; self } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn shared_ref_setter_passes() {
        assert_eq!(count("struct R; impl R { fn set_b(&self) {} }"), 0);
    }

    #[test]
    fn non_setter_mutation_passes() {
        let source = "struct R { b: u8 } impl R { fn update(&mut self, b: u8) { self.b = b; } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn bare_set_passes() {
        assert_eq!(count("struct R; impl R { fn set_(&mut self) {} }"), 0);
    }
}
