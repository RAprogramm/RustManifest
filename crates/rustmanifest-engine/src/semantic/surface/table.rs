// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The recovered route table and the report that carries it: one row per
//! endpoint, plus every refusal that kept a registration off the table.

use rustmanifest_schema::Range;

/// One recovered endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// HTTP method in upper case.
    pub method:      String,
    /// Full path, composed through every enclosing prefix.
    pub path:        String,
    /// Handler function as written at the registration.
    pub handler:     String,
    /// Whether the registration, or any composition that mounts it,
    /// stands under a construct that may not run — a branch, a match arm,
    /// a loop, a closure, or a `#[cfg]` gate — so the route exists in some
    /// builds or runs only, and is not presented as unconditional.
    pub conditional: bool,
    /// Index of the source the registration lives in.
    pub source:      usize,
    /// Byte range of the registration call.
    pub range:       Range
}

/// The external surface recovered from one source slice.
#[derive(Debug, Default)]
pub struct Surface {
    /// Recovered endpoints, sorted by path then method.
    pub routes:           Vec<Route>,
    /// Sources that do not parse as Rust, so nothing could be read from
    /// them. A parse failure and a route-free file are different answers.
    pub unparsed:         u32,
    /// Registrations whose path is not a literal — assembled at runtime
    /// and therefore not recoverable by construction.
    pub dynamic:          u32,
    /// Registrations whose path *is* a literal but whose handler argument
    /// is not a recognised method-router, so no verb could be read. The
    /// path is known; the method is not.
    pub opaque_handler:   u32,
    /// Calls using framework vocabulary on a receiver that was never
    /// proven to hold a router value. Counted rather than assumed to be a
    /// registration, and rather than dropped.
    pub foreign_receiver: u32,
    /// Framework-vocabulary calls written inside positions the walk does
    /// not evaluate — the token stream of a macro invocation. What a
    /// macro does with its tokens is the macro's own business, so nothing
    /// is recovered there; counting the calls keeps the loss a number
    /// instead of a silence.
    pub unwalked:         u32,
    /// Composition arguments that name no resolvable provider function,
    /// with the composing method that held them.
    pub unresolved:       Vec<String>,
    /// Composition arguments matching more than one provider with equal
    /// proximity, so the fold target is undetermined and is not guessed.
    pub ambiguous:        Vec<String>,
    /// Compositions whose mount prefix is not a literal: the sub-router
    /// exists and is not a root, but no path can be composed for it, so
    /// its routes are not emitted at all rather than emitted unprefixed.
    pub unplaced:         usize,
    /// Providers folded into other providers, hence not surface roots
    /// themselves.
    pub composed:         usize,
    /// Functions observed to build a router value.
    pub providers:        usize,
    /// Registrations standing under a construct that may not run. Counted
    /// once per registration call, not once per endpoint it expands to.
    pub conditional:      usize,
    /// `METHOD path` pairs claimed by more than one endpoint. A framework
    /// that rejects duplicate routes cannot serve this surface at all.
    pub conflicts:        Vec<String>,
    /// Providers that fold each other, leaving no root to expand from.
    /// Their composition order is undetermined, so no path is composed
    /// for them — the same refusal the layer inference makes on mutual
    /// module dependencies.
    pub entangled:        Vec<String>,
    /// Registrations proven to exist but emitted nowhere because every
    /// provider holding them is part of a composition cycle.
    pub discarded:        usize
}

/// Reports every `METHOD path` claimed by more than one endpoint.
///
/// `routes` must already be sorted by path then method, so competing
/// claims are adjacent.
pub(super) fn conflicts(routes: &[Route]) -> Vec<String> {
    routes
        .chunk_by(|left, right| left.path == right.path && left.method == right.method)
        .filter(|group| group.len() > 1)
        .filter_map(|group| {
            group
                .first()
                .map(|route| format!("{} {}", route.method, route.path))
        })
        .collect()
}
