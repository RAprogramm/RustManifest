// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Configuration gating: the attributes written on a statement, and
//! whether one of them can remove that statement from a build.

/// Attributes written on an expression, for every shape an expression
/// can take. `syn` exposes them per variant only, and a partial match
/// silently loses the gate on the shapes it omits. The variant list is
/// kept one shape per line: a formatter that spreads each pattern over
/// three lines buries the list it is here to make readable.
#[rustfmt::skip]
fn expression_attributes(expression: &syn::Expr) -> &[syn::Attribute] {
    match expression {
        syn::Expr::Array(syn::ExprArray { attrs, .. })
        | syn::Expr::Assign(syn::ExprAssign { attrs, .. })
        | syn::Expr::Async(syn::ExprAsync { attrs, .. })
        | syn::Expr::Await(syn::ExprAwait { attrs, .. })
        | syn::Expr::Binary(syn::ExprBinary { attrs, .. })
        | syn::Expr::Block(syn::ExprBlock { attrs, .. })
        | syn::Expr::Break(syn::ExprBreak { attrs, .. })
        | syn::Expr::Call(syn::ExprCall { attrs, .. })
        | syn::Expr::Cast(syn::ExprCast { attrs, .. })
        | syn::Expr::Closure(syn::ExprClosure { attrs, .. })
        | syn::Expr::Const(syn::ExprConst { attrs, .. })
        | syn::Expr::Continue(syn::ExprContinue { attrs, .. })
        | syn::Expr::Field(syn::ExprField { attrs, .. })
        | syn::Expr::ForLoop(syn::ExprForLoop { attrs, .. })
        | syn::Expr::Group(syn::ExprGroup { attrs, .. })
        | syn::Expr::If(syn::ExprIf { attrs, .. })
        | syn::Expr::Index(syn::ExprIndex { attrs, .. })
        | syn::Expr::Infer(syn::ExprInfer { attrs, .. })
        | syn::Expr::Let(syn::ExprLet { attrs, .. })
        | syn::Expr::Lit(syn::ExprLit { attrs, .. })
        | syn::Expr::Loop(syn::ExprLoop { attrs, .. })
        | syn::Expr::Macro(syn::ExprMacro { attrs, .. })
        | syn::Expr::Match(syn::ExprMatch { attrs, .. })
        | syn::Expr::MethodCall(syn::ExprMethodCall { attrs, .. })
        | syn::Expr::Paren(syn::ExprParen { attrs, .. })
        | syn::Expr::Path(syn::ExprPath { attrs, .. })
        | syn::Expr::Range(syn::ExprRange { attrs, .. })
        | syn::Expr::RawAddr(syn::ExprRawAddr { attrs, .. })
        | syn::Expr::Reference(syn::ExprReference { attrs, .. })
        | syn::Expr::Repeat(syn::ExprRepeat { attrs, .. })
        | syn::Expr::Return(syn::ExprReturn { attrs, .. })
        | syn::Expr::Struct(syn::ExprStruct { attrs, .. })
        | syn::Expr::Try(syn::ExprTry { attrs, .. })
        | syn::Expr::TryBlock(syn::ExprTryBlock { attrs, .. })
        | syn::Expr::Tuple(syn::ExprTuple { attrs, .. })
        | syn::Expr::Unary(syn::ExprUnary { attrs, .. })
        | syn::Expr::Unsafe(syn::ExprUnsafe { attrs, .. })
        | syn::Expr::While(syn::ExprWhile { attrs, .. })
        | syn::Expr::Yield(syn::ExprYield { attrs, .. }) => attrs,
        _ => &[]
    }
}

/// Attributes written on an item, for every shape an item can take.
#[rustfmt::skip]
fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(syn::ItemConst { attrs, .. })
        | syn::Item::Enum(syn::ItemEnum { attrs, .. })
        | syn::Item::ExternCrate(syn::ItemExternCrate { attrs, .. })
        | syn::Item::Fn(syn::ItemFn { attrs, .. })
        | syn::Item::ForeignMod(syn::ItemForeignMod { attrs, .. })
        | syn::Item::Impl(syn::ItemImpl { attrs, .. })
        | syn::Item::Macro(syn::ItemMacro { attrs, .. })
        | syn::Item::Mod(syn::ItemMod { attrs, .. })
        | syn::Item::Static(syn::ItemStatic { attrs, .. })
        | syn::Item::Struct(syn::ItemStruct { attrs, .. })
        | syn::Item::Trait(syn::ItemTrait { attrs, .. })
        | syn::Item::TraitAlias(syn::ItemTraitAlias { attrs, .. })
        | syn::Item::Type(syn::ItemType { attrs, .. })
        | syn::Item::Union(syn::ItemUnion { attrs, .. })
        | syn::Item::Use(syn::ItemUse { attrs, .. }) => attrs,
        _ => &[]
    }
}

/// Attributes written on a statement, whatever shape it takes.
fn statement_attributes(statement: &syn::Stmt) -> &[syn::Attribute] {
    match statement {
        syn::Stmt::Local(local) => &local.attrs,
        syn::Stmt::Item(item) => item_attributes(item),
        syn::Stmt::Expr(expression, _) => expression_attributes(expression),
        syn::Stmt::Macro(invocation) => &invocation.attrs
    }
}

/// Returns whether an attribute can remove the statement it stands on
/// from a build.
///
/// `cfg` can. `cfg_attr` cannot: it applies an *attribute*
/// conditionally to a statement that is present in every build, so
/// reading it as a gate would weaken an unconditional write and let a
/// genuine divergence stop killing what follows it. The one spelling
/// that does gate is `cfg_attr` applying a `cfg`, which is read here
/// rather than assumed away. An attribute whose arguments do not parse
/// keeps the weakening, the direction that claims less.
fn configuration_gate(attribute: &syn::Attribute) -> bool {
    if attribute.path().is_ident("cfg") {
        return true;
    }
    if !attribute.path().is_ident("cfg_attr") {
        return false;
    }
    let Ok(applied) = attribute.parse_args_with(
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
    ) else {
        return true;
    };
    applied
        .iter()
        .skip(1)
        .any(|meta| meta.path().is_ident("cfg"))
}

/// Returns whether a statement is gated by a configuration attribute
/// this tier does not resolve.
pub(super) fn gated(statement: &syn::Stmt) -> bool {
    statement_attributes(statement)
        .iter()
        .any(configuration_gate)
}
