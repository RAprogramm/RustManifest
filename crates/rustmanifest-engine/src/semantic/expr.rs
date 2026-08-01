// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Shared expression-shape utilities for the semantic tier.
//!
//! Every check that reasons about `syn` expressions needs the same small
//! vocabulary: what a place expression is, what its root identifier is,
//! which operators are compound assignments, which methods are known to
//! mutate or consume their receiver. Those answers live here once, so a
//! new check reuses them instead of restating them.

/// Mutating methods that count as a write to the place they are called on.
pub(super) const MUTATING_METHODS: [&str; 12] = [
    "borrow_mut",
    "clear",
    "get_mut",
    "insert",
    "lock",
    "push",
    "remove",
    "replace",
    "set",
    "store",
    "take",
    "write"
];

/// Renders a place expression (`root`, `root.field`, `self.field.inner`)
/// or returns `None` for anything that is not a plain place.
pub(super) fn place_chain(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => {
            let base = place_chain(&field.base)?;
            let member = match &field.member {
                syn::Member::Named(ident) => ident.to_string(),
                syn::Member::Unnamed(index) => index.index.to_string()
            };
            Some(format!("{base}.{member}"))
        }
        _ => None
    }
}

/// Returns the root identifier of a place expression, seeing through
/// field access, indexing, parentheses, and dereference.
pub(super) fn place_root(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => place_root(&field.base),
        syn::Expr::Index(index) => place_root(&index.expr),
        syn::Expr::Paren(paren) => place_root(&paren.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            place_root(&unary.expr)
        }
        syn::Expr::MethodCall(call) => place_root(&call.receiver),
        _ => None
    }
}

/// Extracts the identifier a `let` pattern binds, `mut` or not.
pub(super) fn binding_ident(pattern: &syn::Pat) -> Option<&syn::Ident> {
    match pattern {
        syn::Pat::Ident(ident) => Some(&ident.ident),
        syn::Pat::Type(typed) => binding_ident(&typed.pat),
        _ => None
    }
}

/// Returns whether `method` consumes its receiver.
pub(super) fn consuming_method(method: &str) -> bool {
    method.starts_with("into") || method == "leak"
}

/// Returns whether `op` is a compound assignment operator.
pub(super) const fn compound_assignment(op: syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_)
            | syn::BinOp::BitXorAssign(_)
            | syn::BinOp::BitAndAssign(_)
            | syn::BinOp::BitOrAssign(_)
            | syn::BinOp::ShlAssign(_)
            | syn::BinOp::ShrAssign(_)
    )
}

/// Renders the last path segment of an impl's self type.
pub(super) fn impl_type_name(block: &syn::ItemImpl) -> Option<String> {
    let syn::Type::Path(path) = block.self_ty.as_ref() else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// Splits macro token text into identifier-shaped words.
pub(super) fn token_words(tokens: &str) -> Vec<String> {
    tokens
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{binding_ident, consuming_method, place_chain, place_root, token_words};

    fn expr(text: &str) -> syn::Expr {
        syn::parse_str(text).unwrap()
    }

    #[test]
    fn place_chains_render_field_paths() {
        assert_eq!(place_chain(&expr("config")).as_deref(), Some("config"));
        assert_eq!(
            place_chain(&expr("self.config.retries")).as_deref(),
            Some("self.config.retries")
        );
        assert_eq!(place_chain(&expr("pair.0")).as_deref(), Some("pair.0"));
        assert_eq!(place_chain(&expr("make()")), None);
    }

    #[test]
    fn place_roots_see_through_wrappers() {
        assert_eq!(
            place_root(&expr("(*items)[0].len()")).as_deref(),
            Some("items")
        );
        assert_eq!(place_root(&expr("42")), None);
    }

    #[test]
    fn binding_idents_unwrap_type_ascription() {
        let pattern: syn::Pat = syn::parse_str::<syn::Stmt>("let value: u32 = 1;").map_or_else(
            |_| panic!("statement"),
            |statement| match statement {
                syn::Stmt::Local(local) => local.pat,
                _ => panic!("local")
            }
        );
        assert_eq!(
            binding_ident(&pattern).map(ToString::to_string).as_deref(),
            Some("value")
        );
    }

    #[test]
    fn consuming_methods_are_recognized() {
        assert!(consuming_method("into_bytes"));
        assert!(consuming_method("leak"));
        assert!(!consuming_method("len"));
    }

    #[test]
    fn token_words_split_on_non_identifier_characters() {
        assert_eq!(
            token_words("\"{}\", snapshot.retries"),
            ["snapshot", "retries"]
        );
    }
}
