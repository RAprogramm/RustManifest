// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How a request over the wire asks for the records.
//!
//! A recovered endpoint whose handler touches the store is a way to
//! ask: the method and path are what a caller writes, and the verb the
//! handler performs on the records is what they get. Both halves are
//! already derived — the route table from the registrations, the verb
//! from what the handler does to the field — so the reading joins them
//! and adds nothing.
//!
//! An endpoint whose handler never touches the records is not listed
//! here. A server may serve for a hundred reasons, and only the routes
//! that reach the records are about them.

use super::verb::{Verb, keeping};
use crate::semantic::{surface::surface, workspace::Workspace};

/// One endpoint that reaches the records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serving {
    /// HTTP method in upper case.
    pub method:      String,
    /// Full path as the route table composed it.
    pub path:        String,
    /// The store the handler touches, as `Owner.field`.
    pub store:       String,
    /// Handler function the route reaches.
    pub reaches:     String,
    /// What that handler does to the records.
    pub verb:        Verb,
    /// The one parameter the path writes, where the handler provably
    /// asks the store with it: the path spells exactly one `{name}`,
    /// the handler's signature takes a path-extractor type apart into
    /// one binding, and the handler's own lookup into the store is
    /// written with that very binding. Any link short of that, and no
    /// name is claimed.
    pub named_by:    Option<String>,
    /// Whether the registration stands under a construct that may not
    /// run, so the route exists in some builds or runs only.
    pub conditional: bool
}

/// Every recovered endpoint whose handler performs a verb on a store.
///
/// In route-table order — path, then method — so two runs over one
/// workspace state them alike.
#[must_use]
pub fn servings(workspace: &Workspace<'_>) -> Vec<Serving> {
    let kept = keeping(workspace);
    let held = super::store::stores(workspace);
    let takers = super::taker::path_takers(workspace.sources());
    let sources = workspace.sources();
    let facts = workspace.facts();
    let qualifiers = crate::semantic::meaning::qualifiers_by_fact(sources, facts);
    let composed = crate::semantic::summary::composition_with(
        sources,
        workspace.resolution(),
        facts,
        workspace.calls(),
        &qualifiers
    );
    let mut served = Vec::new();
    for route in surface(sources).routes {
        let Some(reached) = kept
            .iter()
            .find(|entry| handler_reaches(&route.handler, &entry.function))
        else {
            continue;
        };
        let asks = asked_with_the_path(&takers, &held, facts, &composed, reached);
        served.push(Serving {
            method:      route.method,
            path:        route.path.clone(),
            store:       reached.store.clone(),
            reaches:     reached.function.clone(),
            verb:        reached.verb,
            named_by:    path_parameter(&route.path).filter(|_| asks),
            conditional: route.conditional
        });
    }
    served
}

/// Methods a run asks for one record with: a keyed read, a keyed write
/// access, a removal.
const ASKING: [&str; 3] = ["get", "get_mut", "remove"];

/// Whether the handler's own lookup into the store is written with the
/// very name its path-extractor pattern binds.
///
/// Three links, each read and none guessed: the binding the pattern
/// introduces, the call site asking the collection by a key, and the
/// derivation tying that site's written argument back to the handler's
/// own input of that name.
fn asked_with_the_path(
    takers: &[(String, String)],
    held: &[super::store::Store],
    facts: &[crate::semantic::behavior::FunctionFacts],
    composed: &crate::semantic::summary::Composition<'_>,
    reached: &crate::semantic::keeping::Keeping
) -> bool {
    let Some((_, binding)) = takers
        .iter()
        .find(|(name, _)| handler_reaches(&reached.function, name))
    else {
        return false;
    };
    let Some(shape) = held
        .iter()
        .find(|store| format!("{}.{}", store.owner, store.field) == reached.store)
        .map(|store| store.shape.as_str())
    else {
        return false;
    };
    let Some(index) = facts.iter().position(|fact| {
        let same_name = fact.function == reached.function;
        same_name && fact.range == reached.site
    }) else {
        return false;
    };
    let Some(fact) = facts.get(index) else {
        return false;
    };
    fact.call_sites.iter().any(|site| {
        ASKING
            .iter()
            .any(|method| site.candidate == format!("{shape}::{method}"))
            && site
                .arguments
                .written()
                .and_then(|written| written.get(1))
                .is_some_and(|origin| {
                    matches!(
                        composed.of(index, origin),
                        crate::semantic::summary::ComposedOrigin::Parameter(slot)
                            if slot.name.as_deref() == Some(binding)
                    )
                })
    })
}

/// The one parameter the path writes, where it writes exactly one.
fn path_parameter(path: &str) -> Option<String> {
    let mut written = path
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split('}').next());
    let (first, rest) = (written.next()?, written.next());
    if rest.is_some() {
        return None;
    }
    Some(first.trim_start_matches(':').to_owned())
}

/// Whether a handler written at a registration names the function a
/// keeping was read from.
fn handler_reaches(handler: &str, function: &str) -> bool {
    handler == function
        || handler
            .rsplit("::")
            .next()
            .is_some_and(|named| function.rsplit("::").next() == Some(named))
}
