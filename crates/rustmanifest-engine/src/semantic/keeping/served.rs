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
    let mut served = Vec::new();
    for route in surface(workspace.sources()).routes {
        let Some(reached) = kept
            .iter()
            .find(|entry| handler_reaches(&route.handler, &entry.function))
        else {
            continue;
        };
        served.push(Serving {
            method:      route.method,
            path:        route.path,
            store:       reached.store.clone(),
            reaches:     reached.function.clone(),
            verb:        reached.verb,
            conditional: route.conditional
        });
    }
    served
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
