// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Output renderers for `rustmanifest` findings.
//!
//! Three concrete renderers all share the [`Renderer`] trait so callers can
//! select format at runtime: [`json::JsonRenderer`] for canonical
//! machine-readable output, [`sarif::SarifRenderer`] for SARIF 2.1.0 (used by
//! GitHub code scanning and other CI integrations), and [`tty::TtyRenderer`]
//! for human-readable terminal output with optional color.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

pub mod error;
pub mod json;
pub mod sarif;
pub mod tty;

use rustmanifest_schema::Finding;

pub use crate::{error::ReportError, json::JsonRenderer, sarif::SarifRenderer, tty::TtyRenderer};

/// Trait implemented by every output renderer.
pub trait Renderer {
    /// Renders a slice of findings to the destination owned by the renderer.
    ///
    /// # Errors
    ///
    /// Returns an [`AppError`] if rendering or writing to the destination
    /// fails.
    fn render(&mut self, findings: &[Finding]) -> Result<(), masterror::AppError>;
}
