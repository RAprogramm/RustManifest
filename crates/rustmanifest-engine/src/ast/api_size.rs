// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `api-size` check: a structure's public interface has at most five
//! methods; `new` and `default` are excluded (`STRUCTURE.md` §4).

use syn::spanned::Spanned;

use super::{Occurrence, flattened};

/// Maximum number of counted public methods per inherent impl block.
const METHOD_LIMIT: usize = 5;

/// Constructor names excluded from the count.
const EXCLUDED: [&str; 2] = ["new", "default"];

/// Reports inherent impl blocks exposing more than [`METHOD_LIMIT`] public
/// methods. Trait impls never count: the trait, not the type, owns their
/// surface.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        let syn::Item::Impl(block) = item else {
            continue;
        };
        if block.trait_.is_some() {
            continue;
        }
        let count = block
            .items
            .iter()
            .filter_map(|member| match member {
                syn::ImplItem::Fn(method) => Some(method),
                _ => None
            })
            .filter(|method| matches!(method.vis, syn::Visibility::Public(_)))
            .filter(|method| {
                let name = method.sig.ident.to_string();
                !EXCLUDED.contains(&name.as_str())
            })
            .count();
        if count > METHOD_LIMIT {
            found.push(Occurrence::new(
                &block.self_ty.span(),
                format!(
                    "inherent impl of `{}` exposes {count} public methods (limit {METHOD_LIMIT}, \
                     `new`/`default` excluded); extract secondary concerns into separate types",
                    type_name(&block.self_ty)
                )
            ));
        }
    }
    found
}

/// Renders the implemented type's name for the message.
fn type_name(ty: &syn::Type) -> String {
    if let syn::Type::Path(path) = ty
        && let Some(segment) = path.path.segments.last()
    {
        return segment.ident.to_string();
    }
    "this type".to_owned()
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

    fn impl_with(methods: &str) -> String {
        format!("struct D; impl D {{ {methods} }}")
    }

    #[test]
    fn oversized_public_api_is_reported() {
        let methods = "pub fn a(&self) {} pub fn b(&self) {} pub fn c(&self) {} pub fn d(&self) \
                       {} pub fn e(&self) {} pub fn f(&self) {}";
        assert_eq!(count(&impl_with(methods)), 1);
    }

    #[test]
    fn five_methods_pass() {
        let methods = "pub fn a(&self) {} pub fn b(&self) {} pub fn c(&self) {} pub fn d(&self) \
                       {} pub fn e(&self) {}";
        assert_eq!(count(&impl_with(methods)), 0);
    }

    #[test]
    fn new_default_and_private_methods_are_not_counted() {
        let methods = "pub fn new() -> Self { Self } pub fn default() -> Self { Self } fn g(&self) \
                       {} pub fn a(&self) {} pub fn b(&self) {} pub fn c(&self) {} pub fn d(&self) \
                       {} pub fn e(&self) {}";
        assert_eq!(count(&impl_with(methods)), 0);
    }

    #[test]
    fn trait_impls_are_not_counted() {
        let source = "struct D; trait T { fn a(&self); } impl T for D { fn a(&self) {} }";
        assert_eq!(count(source), 0);
    }
}
