// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Configuration loader for `rustmanifest`.
//!
//! Phase 0 re-exports the canonical [`rustmanifest_schema::Config`] type.
//! Parsing of `rustmanifest.toml`, profile resolution, glob filtering, and
//! inline pragma handling land in later phases.

pub use rustmanifest_schema::{Config, RuleOverride};
