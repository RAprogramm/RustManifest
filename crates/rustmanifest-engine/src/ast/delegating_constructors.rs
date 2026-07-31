// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `delegating-constructors` check: one primary constructor builds `Self`,
//! convenience constructors delegate to it (`STRUCTURE.md` §6).
//!
//! A constructor is an associated function without a `self` receiver whose
//! return type mentions `Self`. When two or more constructors in one inherent
//! impl block each contain a literal `Self { .. }` expression, field
//! initialization is duplicated and the impl block is reported.

use syn::{spanned::Spanned, visit::Visit};

use super::{Occurrence, flattened};

/// Reports inherent impl blocks whose constructors each build `Self { .. }`
/// literally instead of delegating to one primary constructor.
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
        let literal_builders = block
            .items
            .iter()
            .filter_map(|member| match member {
                syn::ImplItem::Fn(method) => Some(method),
                _ => None,
            })
            .filter(|method| constructor(method))
            .filter(|method| builds_self_literally(&method.block))
            .count();
        if literal_builders >= 2 {
            found.push(Occurrence::new(
                &block.self_ty.span(),
                format!(
                    "inherent impl of `{}` has {literal_builders} constructors each building \
                     `Self {{ .. }}` literally; keep one primary constructor and delegate the \
                     rest to it",
                    type_name(&block.self_ty)
                ),
            ));
        }
    }
    found
}

/// True for associated functions without a receiver whose return type
/// mentions `Self`.
fn constructor(method: &syn::ImplItemFn) -> bool {
    method.sig.receiver().is_none() && mentions_self(&method.sig.output)
}

/// True when the return type contains the `Self` identifier anywhere,
/// covering `Self`, `Result<Self, E>`, `Option<Self>`, and similar shapes.
fn mentions_self(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else {
        return false;
    };
    let mut scan = SelfMention { found: false };
    scan.visit_type(ty);
    scan.found
}

/// True when the block contains a `Self { .. }` struct literal.
fn builds_self_literally(block: &syn::Block) -> bool {
    let mut scan = SelfLiteral { found: false };
    scan.visit_block(block);
    scan.found
}

/// Type visitor looking for the `Self` identifier.
struct SelfMention {
    /// Whether `Self` was seen.
    found: bool,
}

impl<'ast> Visit<'ast> for SelfMention {
    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        if node == "Self" {
            self.found = true;
        }
    }
}

/// Expression visitor looking for a `Self { .. }` struct literal.
struct SelfLiteral {
    /// Whether a `Self { .. }` literal was seen.
    found: bool,
}

impl<'ast> Visit<'ast> for SelfLiteral {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if node.qself.is_none() && node.path.is_ident("Self") {
            self.found = true;
        }
        syn::visit::visit_expr_struct(self, node);
    }
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

    #[test]
    fn duplicated_literal_constructors_are_reported() {
        let source = "struct S { a: u8 } impl S { fn new() -> Self { Self { a: 0 } } fn zeroed() \
                      -> Self { Self { a: 0 } } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn fallible_literal_constructors_are_reported() {
        let source = "struct S { a: u8 } impl S { fn new() -> Self { Self { a: 0 } } fn checked() \
                      -> Result<Self, ()> { Ok(Self { a: 1 }) } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn single_constructor_passes() {
        let source = "struct S { a: u8 } impl S { fn new(a: u8) -> Self { Self { a } } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn delegating_convenience_constructors_pass() {
        let source = "struct S { a: u8 } impl S { fn with(a: u8) -> Self { Self { a } } fn new() \
                      -> Self { Self::with(0) } fn one() -> Self { Self::with(1) } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn methods_with_receiver_are_not_constructors() {
        let source = "struct S { a: u8 } impl S { fn new(a: u8) -> Self { Self { a } } fn \
                      duplicated(&self) -> Self { Self { a: self.a } } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn trait_impls_pass() {
        let source = "struct S { a: u8 } trait Make { fn new() -> Self; fn zeroed() -> Self; } \
                      impl Make for S { fn new() -> Self { Self { a: 0 } } fn zeroed() -> Self { \
                      Self { a: 0 } } }";
        assert_eq!(count(source), 0);
    }
}
