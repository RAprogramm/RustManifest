// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Matching a composition argument against the collected providers: a
//! suffix match on the full path, nearest module wins, a tie reported as
//! ambiguous rather than broken arbitrarily.

use std::collections::BTreeMap;

use super::{provider::Provider, table::Surface};

/// A composition whose target resolved to exactly one provider key.
#[derive(Debug)]
pub(super) struct Fold {
    /// Mount prefix; `None` when the prefix argument is not a literal.
    pub(super) prefix:      Option<String>,
    /// Key of the folded provider.
    pub(super) target:      String,
    /// Whether the composition stands under a construct that may not run.
    pub(super) conditional: bool
}

/// What a composition argument resolved to.
enum Resolution {
    /// Exactly one provider is the nearest match.
    Unique(String),
    /// No provider carries the referenced path as a suffix.
    Missing,
    /// Several providers match with equal proximity.
    Ambiguous
}

/// Resolves every provider's compositions, recording the ones that name
/// no provider and the ones that name several.
pub(super) fn resolved_folds(
    providers: &BTreeMap<String, Provider>,
    recovered: &mut Surface
) -> BTreeMap<String, Vec<Fold>> {
    let mut out = BTreeMap::new();
    for (key, provider) in providers {
        let mut folds = Vec::new();
        for raw in &provider.folds {
            match resolve(&raw.reference, &provider.scope, providers) {
                Resolution::Unique(target) => folds.push(Fold {
                    prefix: raw.prefix.clone(),
                    target,
                    conditional: raw.conditional
                }),
                Resolution::Missing => recovered.unresolved.push(raw.method.clone()),
                Resolution::Ambiguous => recovered.ambiguous.push(raw.reference.join("::"))
            }
        }
        out.insert(key.clone(), folds);
    }
    out
}

/// Resolves a written path against the collected providers.
///
/// A provider matches when its full path ends with `reference`, so
/// `api::routes` answers both `routes()` and `api::routes()`. Among the
/// matches, the one sharing the longest module prefix with `scope` wins;
/// a tie is ambiguous and is reported rather than broken arbitrarily.
fn resolve(
    reference: &[String],
    origin: &[String],
    providers: &BTreeMap<String, Provider>
) -> Resolution {
    let mut best: Option<(usize, &String)> = None;
    let mut tied = false;
    for (key, provider) in providers {
        if !provider.path.ends_with(reference) {
            continue;
        }
        let nearness = shared(&provider.scope, origin);
        let Some((top, _)) = best else {
            best = Some((nearness, key));
            tied = false;
            continue;
        };
        if nearness > top {
            best = Some((nearness, key));
            tied = false;
        } else if nearness == top {
            tied = true;
        }
    }
    match best {
        None => Resolution::Missing,
        Some(_) if tied => Resolution::Ambiguous,
        Some((_, key)) => Resolution::Unique(key.clone())
    }
}

/// Length of the common leading run of two module paths.
fn shared(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(one, other)| one == other)
        .count()
}
