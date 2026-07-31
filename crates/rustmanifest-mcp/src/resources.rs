// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Methodology resources re-exported from the rules pack.
//!
//! The document table lives in `rustmanifest-rules-core::methodology` so the
//! CLI and the MCP server share one source of truth; this module only
//! re-exports it under the server's historical path.

pub use rustmanifest_rules_core::methodology::{
    DOCS, MethodologyDoc, base_uri, find, rationale_excerpt,
};
