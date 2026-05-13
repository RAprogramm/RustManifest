// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Default rules pack for `rustmanifest`.
//!
//! Rules are authored as TOML files under `rules/<RULE-ID>/rule.toml`
//! alongside `pass.rs` and `fail.rs` fixtures. At build time the
//! `rustmanifest-rules-core` build script walks `rules/`, validates the
//! definitions, and emits a single canonical `rules.json` into `OUT_DIR`.
//! Runtime code accesses the parsed rules through the [`RULES`] lazily
//! initialized list.

use std::sync::LazyLock;

use rustmanifest_schema::Rule;

/// Identifier of this rules pack, namespaced into rule IDs (e.g. `RM-SEC-001`).
pub const PACK_ID: &str = "rm";

/// Semantic version of this rules pack, independent of the engine version.
pub const PACK_VERSION: &str = env!("CARGO_PKG_VERSION");

const RULES_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/rules.json"));

/// All rules bundled in this pack, parsed once on first access.
///
/// Iteration order matches the order produced by the build script (sorted by
/// directory name, which equals the rule id), so the order is deterministic
/// across builds and processes.
pub static RULES: LazyLock<Vec<Rule>> = LazyLock::new(parse_embedded_rules);

#[allow(
    clippy::expect_used,
    reason = "RULES_JSON is generated and validated by build.rs; a parse failure here means \
              the compiled binary has been corrupted post-link, which is unrecoverable."
)]
fn parse_embedded_rules() -> Vec<Rule> {
    serde_json::from_str(RULES_JSON)
        .expect("rustmanifest-rules-core: embedded rules.json is malformed")
}
