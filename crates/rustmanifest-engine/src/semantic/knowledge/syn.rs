// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge pack for the `syn` crate: the written form of Rust itself,
//! as a tree a program reads.
//!
//! What matters here is the shape of the tree, not what any call does to
//! the world — reading a syntax tree touches nothing, so the pack
//! carries no effects at all. The rows name the fields a reader walks
//! most: the attributes written above an item and the items written
//! inside a block, each a sequence whose step is what the written loop
//! binds.
//!
//! A method whose returned type is spelled the same as a library type of
//! a different meaning is deliberately absent: this reading names types
//! by their last segment, and a row that put such a name in the tables
//! would let a call on the tree read as a call on the library's
//! namesake.

use super::CrateKnowledge;

/// What the engine knows about `syn`.
pub(super) const PACK: CrateKnowledge = CrateKnowledge {
    name:           "syn",
    constructors:   &[],
    method_returns: &[],
    fields:         &[
        ("ItemFn", "sig", "Signature"),
        ("ItemFn", "block", "Block"),
        ("ItemFn", "vis", "Visibility"),
        ("ImplItemFn", "sig", "Signature"),
        ("TraitItemFn", "sig", "Signature"),
        ("Signature", "ident", "Ident"),
        ("ItemStruct", "ident", "Ident"),
        ("ItemEnum", "ident", "Ident"),
        ("ItemImpl", "self_ty", "Type"),
        ("ExprMethodCall", "receiver", "Expr"),
        ("ExprMethodCall", "method", "Ident"),
        ("ExprCall", "func", "Expr")
    ],
    field_elements: &[
        ("ItemFn", "attrs", "Attribute"),
        ("ItemStruct", "attrs", "Attribute"),
        ("ItemEnum", "attrs", "Attribute"),
        ("ItemMod", "attrs", "Attribute"),
        ("ItemImpl", "attrs", "Attribute"),
        ("ItemImpl", "items", "ImplItem"),
        ("ItemTrait", "items", "TraitItem"),
        ("Block", "stmts", "Stmt"),
        ("ItemStruct", "fields", "Field"),
        ("ItemEnum", "variants", "Variant"),
        ("Signature", "inputs", "FnArg"),
        ("ExprMethodCall", "args", "Expr"),
        ("ExprCall", "args", "Expr"),
        ("File", "items", "Item")
    ],
    iterated:       &[],
    method_effects: &[],
    crate_effect:   None
};
