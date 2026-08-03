// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Resolution through the trait machinery: dynamic dispatch over a
//! trait's implementors and the extension-trait default that grants a
//! method to a concrete owner.

use std::collections::BTreeMap;

use super::context::ResolutionContext;

/// Resolves an unmatched `Owner::method` candidate through the trait
/// machinery: dynamic dispatch over the owner's implementors when the
/// owner names a trait — falling through extension traits to their
/// supertrait's implementors — or the unique extension-trait default
/// granting `method` to a concrete owner.
pub(super) fn dispatch_targets(
    owner: &str,
    method: &str,
    context: &ResolutionContext,
    by_qualified: &BTreeMap<&str, Vec<usize>>
) -> Vec<usize> {
    let implementors = implementors_of(owner, context);
    if let Some(implementors) = implementors {
        let mut targets = Vec::new();
        for implementor in implementors {
            let qualified = format!("{implementor}::{method}");
            if let Some([only]) = by_qualified.get(qualified.as_str()).map(Vec::as_slice) {
                targets.push(*only);
            }
        }
        if !targets.is_empty() {
            return targets;
        }
    }
    ext_target(owner, method, context, by_qualified)
        .map(|target| vec![target])
        .unwrap_or_default()
}

/// Implementors of a trait, seen through the extension-trait pattern:
/// a trait with no direct impls falls through its supertrait chain,
/// bounded so a cycle cannot loop.
fn implementors_of<'context>(
    owner: &str,
    context: &'context ResolutionContext
) -> Option<&'context Vec<String>> {
    let mut current = owner;
    for _ in 0..4 {
        if let Some(implementors) = context.trait_impls.get(current) {
            return Some(implementors);
        }
        current = context.supertraits.get(current)?;
    }
    None
}

/// Resolves an unmatched `Owner::method` candidate to the unique
/// extension-trait default that grants `method` to `Owner`: the owner
/// must implement the extension's supertrait, and the extension's
/// default must exist as exactly one fact. More than one granting
/// extension is ambiguity, and ambiguity stays unresolved.
fn ext_target(
    owner: &str,
    method: &str,
    context: &ResolutionContext,
    by_qualified: &BTreeMap<&str, Vec<usize>>
) -> Option<usize> {
    let entries = context.ext_traits.get(method)?;
    let mut targets = entries.iter().filter_map(|(ext, base)| {
        let grants = context
            .trait_impls
            .get(base)
            .is_some_and(|implementors| implementors.iter().any(|name| name == owner));
        if !grants {
            return None;
        }
        let qualified = format!("{ext}::{method}");
        match by_qualified.get(qualified.as_str()).map(Vec::as_slice) {
            Some([only]) => Some(*only),
            _ => None
        }
    });
    let first = targets.next()?;
    targets.next().is_none().then_some(first)
}
