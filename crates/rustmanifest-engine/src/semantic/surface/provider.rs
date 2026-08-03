// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One function that builds a router value, and the walk that fills it
//! from the item's body.

use std::collections::BTreeMap;

use syn::visit::Visit;

use super::{
    table::{Route, Surface},
    unit::{Body, Unit},
    walk::Walk
};

/// One function that builds a router value.
#[derive(Debug)]
pub(super) struct Provider {
    /// Full path of the item, module path included.
    pub(super) path:   Vec<String>,
    /// Path of the item's owner: `path` without its last segment.
    pub(super) scope:  Vec<String>,
    /// Routes registered directly in this function.
    pub(super) routes: Vec<Route>,
    /// Compositions this function performs, before target resolution.
    pub(super) folds:  Vec<RawFold>
}

/// A composition as written, before its argument is resolved.
#[derive(Debug)]
pub(super) struct RawFold {
    /// Mount prefix; `None` when the prefix argument is not a literal,
    /// `Some("")` for a flat merge.
    pub(super) prefix:      Option<String>,
    /// Path segments naming the folded provider, as written.
    pub(super) reference:   Vec<String>,
    /// Whether the composition stands under a construct that may not run.
    pub(super) conditional: bool,
    /// Composing method, for reporting an unresolvable argument.
    pub(super) method:      String
}

/// Walks one item body and records what it contributes to the surface,
/// registering it as a provider when it contributes anything.
///
/// Traversal is the syntax visitor's own, so no expression form can be
/// forgotten: a registration inside a branch, a match arm, a closure or
/// a macro-free block is reached exactly like one at the top level. A
/// registration standing under a construct that may not run — including a
/// `#[cfg]` gate on the statement or on the item itself — is recorded as
/// conditional rather than dropped or presented as certain. Items nested
/// in the body are skipped here and walked as providers of their own.
pub(super) fn collect(
    unit: &Unit<'_>,
    source: usize,
    providers: &mut BTreeMap<String, Provider>,
    recovered: &mut Surface
) {
    let scope = unit
        .path
        .split_last()
        .map_or_else(Vec::new, |(_, rest)| rest.to_vec());
    let mut provider = Provider {
        path:   unit.path.clone(),
        scope:  scope.clone(),
        routes: Vec::new(),
        folds:  Vec::new()
    };
    let mut walk = Walk {
        source,
        conditional: u32::from(unit.gated),
        routers: unit.routers.iter().cloned().collect(),
        provider: &mut provider,
        recovered
    };
    match unit.body {
        Body::Block(block) => walk.visit_block(block),
        Body::Value(expression) => walk.visit_expr(expression)
    }
    if provider.routes.is_empty() && provider.folds.is_empty() {
        return;
    }
    let entry = providers
        .entry(unit.path.join("::"))
        .or_insert_with(|| Provider {
            path: unit.path.clone(),
            scope,
            routes: Vec::new(),
            folds: Vec::new()
        });
    entry.routes.extend(provider.routes);
    entry.folds.extend(provider.folds);
}
