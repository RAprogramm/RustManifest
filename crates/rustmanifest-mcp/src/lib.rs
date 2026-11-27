// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! MCP server for `rustmanifest` (stdio transport).
//!
//! The server exposes the review engine over the Model Context Protocol:
//!
//! - **Tools** — `review_file`, `review_path`, `check_rule`, `lint_structure`,
//!   `explain_rule` (see [`server`]).
//! - **Resources** — methodology documents embedded at compile time under
//!   `rustmanifest://methodology/` URIs (see [`resources`]).
//! - **Roots** — the workspace root is the process current directory at
//!   startup; every client path is confined to it (see [`workspace`]).
//!
//! All server-facing strings are English regardless of request language.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the rmcp SDK pins duplicate transitive dependencies (syn 2/3, hashbrown, \
              wit-bindgen) that cannot be deduplicated from this crate"
)]

pub mod analysis;
pub mod knowledge;
pub mod resources;
pub mod server;
pub mod workspace;

/// MCP protocol revision targeted by this server by default.
pub const PROTOCOL_REVISION: &str = "2025-11-25";
