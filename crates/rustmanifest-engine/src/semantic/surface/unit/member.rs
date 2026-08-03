// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Members of an impl block or a trait definition, keyed under the type
//! or trait that owns them so two methods of one name stay two units.

use syn::{ImplItem, TraitItem};

use super::{Body, Unit, nested_units};
use crate::semantic::surface::{
    attribute::cfg_gated,
    binding::{router_inputs, type_name},
    path::appended
};

/// Collects the members of one inherent or trait impl.
pub(super) fn impl_units<'a>(
    block: &'a syn::ItemImpl,
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    let owner =
        type_name(&block.self_ty).map_or_else(|| scope.to_vec(), |name| appended(scope, &name));
    for member in &block.items {
        match member {
            ImplItem::Fn(method) => {
                let gated = gated || cfg_gated(&method.attrs);
                let path = appended(&owner, &method.sig.ident.to_string());
                out.push(Unit {
                    path: path.clone(),
                    body: Body::Block(&method.block),
                    gated,
                    routers: router_inputs(&method.sig)
                });
                nested_units(&method.block, &path, gated, out);
            }
            ImplItem::Const(constant) => out.push(Unit {
                path:    appended(&owner, &constant.ident.to_string()),
                body:    Body::Value(&constant.expr),
                gated:   gated || cfg_gated(&constant.attrs),
                routers: Vec::new()
            }),
            _ => {}
        }
    }
}

/// Collects the defaulted members of one trait definition.
pub(super) fn trait_units<'a>(
    definition: &'a syn::ItemTrait,
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    let owner = appended(scope, &definition.ident.to_string());
    for member in &definition.items {
        match member {
            TraitItem::Fn(method) => {
                let Some(block) = method.default.as_ref() else {
                    continue;
                };
                let gated = gated || cfg_gated(&method.attrs);
                let path = appended(&owner, &method.sig.ident.to_string());
                out.push(Unit {
                    path: path.clone(),
                    body: Body::Block(block),
                    gated,
                    routers: router_inputs(&method.sig)
                });
                nested_units(block, &path, gated, out);
            }
            TraitItem::Const(constant) => {
                let Some((_, value)) = constant.default.as_ref() else {
                    continue;
                };
                out.push(Unit {
                    path:    appended(&owner, &constant.ident.to_string()),
                    body:    Body::Value(value),
                    gated:   gated || cfg_gated(&constant.attrs),
                    routers: Vec::new()
                });
            }
            _ => {}
        }
    }
}
