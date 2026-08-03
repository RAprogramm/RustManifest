// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Enumeration of the items whose body may build a router, each keyed by
//! the full path its module, owner and name compose. A router built by a
//! nested item is that item's, not its host's.

mod member;

use syn::{Expr, Item, visit::Visit};

use self::member::{impl_units, trait_units};
use super::{
    attribute::{cfg_gated, item_attributes},
    binding::router_inputs,
    path::appended
};

/// Body of one item that may build a router.
pub(super) enum Body<'a> {
    /// A function or method body.
    Block(&'a syn::Block),
    /// A `const` or `static` initialiser.
    Value(&'a Expr)
}

/// One item whose body is walked as a candidate provider.
pub(super) struct Unit<'a> {
    /// Full path of the item, module path included.
    pub(super) path:    Vec<String>,
    /// The syntax to walk.
    pub(super) body:    Body<'a>,
    /// Whether the item itself stands behind a `#[cfg]` gate.
    pub(super) gated:   bool,
    /// Parameter names whose declared type is a router.
    pub(super) routers: Vec<String>
}

/// Collects every item whose body may build a router, including items
/// nested inside inline modules, impls, traits and function bodies. A
/// router built by a nested item is that item's, not its host's.
pub(super) fn units<'a>(
    items: &[&'a Item],
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    for item in items {
        let gated = gated || cfg_gated(item_attributes(item));
        match item {
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    let inner: Vec<&Item> = nested.iter().collect();
                    units(
                        &inner,
                        &appended(scope, &module.ident.to_string()),
                        gated,
                        out
                    );
                }
            }
            Item::Fn(function) => {
                let path = appended(scope, &function.sig.ident.to_string());
                out.push(Unit {
                    path: path.clone(),
                    body: Body::Block(&function.block),
                    gated,
                    routers: router_inputs(&function.sig)
                });
                nested_units(&function.block, &path, gated, out);
            }
            Item::Impl(block) => impl_units(block, scope, gated, out),
            Item::Trait(definition) => trait_units(definition, scope, gated, out),
            Item::Const(constant) => out.push(Unit {
                path: appended(scope, &constant.ident.to_string()),
                body: Body::Value(&constant.expr),
                gated,
                routers: Vec::new()
            }),
            Item::Static(item) => out.push(Unit {
                path: appended(scope, &item.ident.to_string()),
                body: Body::Value(&item.expr),
                gated,
                routers: Vec::new()
            }),
            _ => {}
        }
    }
}

/// Collects the items declared inside a function body under that
/// function's own path.
pub(super) fn nested_units<'a>(
    block: &'a syn::Block,
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    let mut harvest = ItemHarvest {
        found: Vec::new()
    };
    harvest.visit_block(block);
    if harvest.found.is_empty() {
        return;
    }
    units(&harvest.found, scope, gated, out);
}

/// Collects items declared inside a block without descending into them:
/// each collected item is walked by [`units`] under its own path.
struct ItemHarvest<'ast> {
    /// Items found at any depth of the block.
    found: Vec<&'ast Item>
}

impl<'ast> Visit<'ast> for ItemHarvest<'ast> {
    fn visit_item(&mut self, item: &'ast Item) {
        self.found.push(item);
    }
}
