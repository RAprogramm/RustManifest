// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge pack for the `reqwest` crate: HTTP client calls over the
//! network.

use super::{super::behavior::Effect, CrateKnowledge};

/// What the engine knows about `reqwest`.
pub(super) const PACK: CrateKnowledge = CrateKnowledge {
    name:           "reqwest",
    method_effects: &[
        ("RequestBuilder", "send", Effect::Network),
        ("Response", "bytes", Effect::Network),
        ("Response", "json", Effect::Network),
        ("Response", "text", Effect::Network)
    ],
    crate_effect:   Some(Effect::Network)
};
