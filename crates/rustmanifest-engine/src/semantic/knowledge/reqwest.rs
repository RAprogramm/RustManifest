// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge pack for the `reqwest` crate: HTTP client calls over the
//! network.

use super::{super::behavior::Effect, CrateKnowledge};

/// What the engine knows about `reqwest`.
pub(super) const PACK: CrateKnowledge = CrateKnowledge {
    name:           "reqwest",
    constructors:   &[],
    method_returns: &[
        ("Client", "delete", "RequestBuilder"),
        ("Client", "get", "RequestBuilder"),
        ("Client", "patch", "RequestBuilder"),
        ("Client", "post", "RequestBuilder"),
        ("Client", "put", "RequestBuilder"),
        ("ClientBuilder", "build", "Client"),
        ("RequestBuilder", "body", "RequestBuilder"),
        ("RequestBuilder", "header", "RequestBuilder"),
        ("RequestBuilder", "json", "RequestBuilder"),
        ("RequestBuilder", "query", "RequestBuilder"),
        ("RequestBuilder", "send", "Response")
    ],
    method_effects: &[
        ("RequestBuilder", "send", Effect::Network),
        ("Response", "bytes", Effect::Network),
        ("Response", "json", Effect::Network),
        ("Response", "text", Effect::Network)
    ],
    crate_effect:   Some(Effect::Network)
};
