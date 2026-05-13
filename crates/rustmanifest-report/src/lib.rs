// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Output renderers for `rustmanifest` findings.
//!
//! Three concrete renderers all share the [`Renderer`] trait so callers can
//! select format at runtime: [`json::JsonRenderer`] for canonical
//! machine-readable output, [`sarif::SarifRenderer`] for SARIF 2.1.0 (used by
//! GitHub code scanning and other CI integrations), and [`tty::TtyRenderer`]
//! for human-readable terminal output with optional color.

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
    /// Returns a [`ReportError`] if rendering or writing to the destination
    /// fails.
    fn render(&mut self, findings: &[Finding]) -> Result<(), ReportError>;
}
