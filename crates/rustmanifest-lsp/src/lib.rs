// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! LSP server for `rustmanifest`.
//!
//! The first shipped surface: the bundled rules run over every opened,
//! changed and saved document, and the findings arrive as diagnostics —
//! each carrying its rule identifier, so the editor's reader can ask
//! `explain` about exactly the rule that fired. Workspace-wide
//! invariants deliberately stay out of this surface until the server
//! reads a workspace rather than single documents.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

mod server;

pub use server::serve;

/// Language Server Protocol specification version targeted by this server.
pub const LSP_SPEC_VERSION: &str = "3.17";
