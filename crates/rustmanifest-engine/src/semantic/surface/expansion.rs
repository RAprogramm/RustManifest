// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Composing the route table from the roots down: every provider a root
//! folds in contributes its routes under the mount prefix that reached
//! it, and a composition cycle is cut on revisit rather than looped.

use std::collections::{BTreeMap, BTreeSet};

use super::{path::join, provider::Provider, resolution::Fold, table::Route};

/// Expansion state: the composed table under construction.
pub(super) struct Expansion<'a> {
    /// Every collected provider, keyed by full path.
    pub(super) providers: &'a BTreeMap<String, Provider>,
    /// Resolved compositions per provider key.
    pub(super) folds:     &'a BTreeMap<String, Vec<Fold>>,
    /// Providers on the current composition chain, to cut cycles.
    pub(super) seen:      BTreeSet<String>,
    /// Composed endpoints so far.
    pub(super) routes:    Vec<Route>,
    /// Compositions skipped because their prefix is not a literal.
    pub(super) unplaced:  usize
}

impl Expansion<'_> {
    /// Expands one provider into fully composed routes.
    pub(super) fn expand(&mut self, name: &str, prefix: &str, conditional: bool) {
        if !self.seen.insert(name.to_owned()) {
            return;
        }
        let table = self.providers;
        if let Some(provider) = table.get(name) {
            for route in &provider.routes {
                self.routes.push(Route {
                    method:      route.method.clone(),
                    path:        join(prefix, &route.path),
                    handler:     route.handler.clone(),
                    conditional: route.conditional || conditional,
                    source:      route.source,
                    range:       route.range
                });
            }
        }
        let folds = self.folds;
        if let Some(folds) = folds.get(name) {
            for fold in folds {
                if let Some(mount) = fold.prefix.as_ref() {
                    let next = join(prefix, mount);
                    self.expand(&fold.target, &next, conditional || fold.conditional);
                } else {
                    self.unplaced += 1;
                }
            }
        }
        self.seen.remove(name);
    }
}
