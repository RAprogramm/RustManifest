// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! LSP server for `rustmanifest`.
//!
//! Phase 0 ships the crate skeleton only. The server reuses the engine and
//! emits diagnostics through the Language Server Protocol; implementation
//! lands in Phase 5.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

/// Language Server Protocol specification version targeted by this server.
pub const LSP_SPEC_VERSION: &str = "3.17";
