// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Default rules pack for `rustmanifest`.
//!
//! Phase 0 ships only the version constant and the pack identifier. Phase 1
//! populates this crate with rules parsed from the English methodology
//! markdown together with pass/fail fixtures.

/// Identifier of this rules pack, namespaced into rule IDs (e.g. `RM-SEC-001`).
pub const PACK_ID: &str = "rm";

/// Semantic version of this rules pack, independent of the engine version.
pub const PACK_VERSION: &str = env!("CARGO_PKG_VERSION");
