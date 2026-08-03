// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one framework call against the model that gave it meaning:
//! a route registration, a composition, or the counted reason neither
//! could be read.

use rustmanifest_schema::Range;
use syn::{Expr, spanned::Spanned};

use super::{
    argument::{literal, method_router, provider_reference},
    framework::FrameworkModel,
    provider::RawFold,
    table::Route,
    walk::Walk
};

/// Records one registration or composition against the model.
pub(super) fn record(
    model: &FrameworkModel,
    method: &str,
    call: &syn::ExprMethodCall,
    walk: &mut Walk<'_>
) {
    if model.registrations.contains(&method) {
        record_registration(model, call, walk);
        return;
    }
    record_composition(model, method, call, walk);
}

/// Records one route registration, or the reason it could not be read.
fn record_registration(model: &FrameworkModel, call: &syn::ExprMethodCall, walk: &mut Walk<'_>) {
    let arguments: Vec<&Expr> = call.args.iter().collect();
    let Some(path) = arguments.first().and_then(|argument| literal(argument)) else {
        walk.recovered.dynamic += 1;
        return;
    };
    let handlers = arguments
        .get(1)
        .map(|argument| method_router(model, argument))
        .unwrap_or_default();
    if handlers.is_empty() {
        walk.recovered.opaque_handler += 1;
        return;
    }
    let conditional = walk.conditional > 0;
    if conditional {
        walk.recovered.conditional += 1;
    }
    let span = call.span().byte_range();
    for (verb, handler) in handlers {
        walk.provider.routes.push(Route {
            method: verb,
            path: path.clone(),
            handler,
            conditional,
            source: walk.source,
            range: Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            }
        });
    }
}

/// Records one composition, or the reason its target could not be read.
///
/// A `nest` whose prefix is not a literal still names its sub-router, so
/// the sub-router is recorded as folded — it is not a surface root — and
/// the missing prefix is carried as `None` so expansion counts it instead
/// of emitting the sub-router's routes at the wrong place.
fn record_composition(
    model: &FrameworkModel,
    method: &str,
    call: &syn::ExprMethodCall,
    walk: &mut Walk<'_>
) {
    let arguments: Vec<&Expr> = call.args.iter().collect();
    let nesting = model.nesting.contains(&method);
    let prefix = if nesting {
        arguments.first().and_then(|argument| literal(argument))
    } else {
        Some(String::new())
    };
    let target = if nesting {
        arguments.get(1)
    } else {
        arguments.first()
    };
    let Some(reference) = target.and_then(|argument| provider_reference(argument)) else {
        walk.recovered.unresolved.push(method.to_owned());
        return;
    };
    walk.provider.folds.push(RawFold {
        prefix,
        reference,
        conditional: walk.conditional > 0,
        method: method.to_owned()
    });
}
