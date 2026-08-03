// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Declaration context of one item: where it sits — free function, impl
//! method, trait default — and what its attributes and visibility
//! declare about it, packed into the flags the facts carry.

use super::types::FunctionFlags;

/// Declaration context of one item: where it sits and what it declares.
pub(super) struct ItemContext<'name> {
    /// Impl self type for methods, `None` for free functions.
    pub(super) impl_type: Option<&'name str>,
    /// Declaration flags of the item.
    pub(super) flags:     FunctionFlags,
    /// Impl-block generic parameters to their first trait bound, so a
    /// generic-typed field resolves as its bound inside the block's
    /// methods.
    pub(super) bounds:    std::collections::BTreeMap<String, String>
}

/// Packs declaration flags from the trait context, attributes, and
/// visibility.
pub(super) fn item_flags(
    trait_impl: bool,
    attrs: &[syn::Attribute],
    public: bool,
    inherited_attr: bool
) -> FunctionFlags {
    let mut bits = 0u8;
    if trait_impl {
        bits |= FunctionFlags::TRAIT_IMPL;
    }
    if is_test(attrs) {
        bits |= FunctionFlags::TEST;
    }
    if public {
        bits |= FunctionFlags::PUBLIC;
    }
    if inherited_attr || is_attributed(attrs) {
        bits |= FunctionFlags::ATTRIBUTED;
    }
    FunctionFlags::from_bits(bits)
}

/// Returns whether any attribute is neither a doc comment nor a `test`
/// harness marker.
pub(super) fn is_attributed(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let last = attr
            .path()
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        !matches!(last.as_deref(), Some("doc" | "test"))
    })
}

/// Returns whether any attribute's last path segment is `test`, covering
/// `#[test]` and `#[tokio::test]`-style harness attributes.
fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
    })
}
