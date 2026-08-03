// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Attribute access for every syntax node that can carry one, and the
//! single question asked of them: is this node configuration-dependent.

use syn::{Attribute, Expr, Item};

/// Whether any attribute makes the item's presence configuration-dependent.
pub(super) fn cfg_gated(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

/// Attributes carried by an item.
pub(super) fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(node) => &node.attrs,
        Item::Enum(node) => &node.attrs,
        Item::ExternCrate(node) => &node.attrs,
        Item::Fn(node) => &node.attrs,
        Item::ForeignMod(node) => &node.attrs,
        Item::Impl(node) => &node.attrs,
        Item::Macro(node) => &node.attrs,
        Item::Mod(node) => &node.attrs,
        Item::Static(node) => &node.attrs,
        Item::Struct(node) => &node.attrs,
        Item::Trait(node) => &node.attrs,
        Item::TraitAlias(node) => &node.attrs,
        Item::Type(node) => &node.attrs,
        Item::Union(node) => &node.attrs,
        Item::Use(node) => &node.attrs,
        _ => &[]
    }
}

/// Attributes carried by an expression.
///
/// `syn` exposes no accessor for these, and a partial match is how a
/// `#[cfg]` on a statement-position expression goes unnoticed, so every
/// variant that owns attributes is listed.
fn expression_attributes(expression: &Expr) -> &[Attribute] {
    match expression {
        Expr::Array(node) => &node.attrs,
        Expr::Assign(node) => &node.attrs,
        Expr::Async(node) => &node.attrs,
        Expr::Await(node) => &node.attrs,
        Expr::Binary(node) => &node.attrs,
        Expr::Block(node) => &node.attrs,
        Expr::Break(node) => &node.attrs,
        Expr::Call(node) => &node.attrs,
        Expr::Cast(node) => &node.attrs,
        Expr::Closure(node) => &node.attrs,
        Expr::Const(node) => &node.attrs,
        Expr::Continue(node) => &node.attrs,
        Expr::Field(node) => &node.attrs,
        Expr::ForLoop(node) => &node.attrs,
        Expr::Group(node) => &node.attrs,
        Expr::If(node) => &node.attrs,
        Expr::Index(node) => &node.attrs,
        Expr::Infer(node) => &node.attrs,
        Expr::Let(node) => &node.attrs,
        Expr::Lit(node) => &node.attrs,
        Expr::Loop(node) => &node.attrs,
        Expr::Macro(node) => &node.attrs,
        Expr::Match(node) => &node.attrs,
        Expr::MethodCall(node) => &node.attrs,
        Expr::Paren(node) => &node.attrs,
        Expr::Path(node) => &node.attrs,
        Expr::Range(node) => &node.attrs,
        Expr::RawAddr(node) => &node.attrs,
        Expr::Reference(node) => &node.attrs,
        Expr::Repeat(node) => &node.attrs,
        Expr::Return(node) => &node.attrs,
        Expr::Struct(node) => &node.attrs,
        Expr::Try(node) => &node.attrs,
        Expr::TryBlock(node) => &node.attrs,
        Expr::Tuple(node) => &node.attrs,
        Expr::Unary(node) => &node.attrs,
        Expr::Unsafe(node) => &node.attrs,
        Expr::While(node) => &node.attrs,
        Expr::Yield(node) => &node.attrs,
        _ => &[]
    }
}

/// Attributes carried by a statement.
pub(super) fn statement_attributes(statement: &syn::Stmt) -> &[Attribute] {
    match statement {
        syn::Stmt::Local(local) => &local.attrs,
        syn::Stmt::Item(item) => item_attributes(item),
        syn::Stmt::Expr(expression, _) => expression_attributes(expression),
        syn::Stmt::Macro(macro_statement) => &macro_statement.attrs
    }
}
