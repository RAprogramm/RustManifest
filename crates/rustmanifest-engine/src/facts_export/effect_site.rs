// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one proving site: which effect, what proved it,
//! and where that stands.
//!
//! The effect list on its own is an assertion a reader has to take. This
//! channel is what makes it checkable, so it belongs in the export beside
//! the list rather than only inside the engine.

use crate::semantic::EffectSite;

/// Renders one effect site as a JSON object.
#[must_use]
pub(super) fn effect_site_json(site: &EffectSite) -> serde_json::Value {
    serde_json::json!({
        "effect": site.effect.to_string(),
        "proof": site.proof,
        "range": { "start": site.range.start, "end": site.range.end }
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{semantic::behavior, source::Source};

    #[test]
    fn a_site_states_its_effect_its_proof_and_its_place() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "pub fn serve() { std::net::TcpListener::bind(\"0\").ok(); }".to_owned()
        )];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let site = facts
            .iter()
            .find(|fact| fact.function == "serve")
            .and_then(|fact| fact.effect_sites.first())
            .expect("the function records a proving site");
        let json = effect_site_json(site);
        assert_eq!(json["effect"], "net");
        assert_eq!(json["proof"], "std::net::TcpListener::bind");
        assert!(json["range"]["end"].as_u64() > json["range"]["start"].as_u64());
    }
}
