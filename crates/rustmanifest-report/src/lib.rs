// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Output renderers for `rustmanifest` findings.
//!
//! Phase 0 defines the renderer trait surface. Concrete `Json`, `Sarif`, and
//! `Tty` renderers land in Phase 1 alongside the engine implementation.

use rustmanifest_schema::Finding;

/// Trait implemented by every output renderer.
pub trait Renderer {
    /// Error type returned by [`Self::render`].
    type Error;

    /// Renders a slice of findings to the destination owned by the renderer.
    ///
    /// # Errors
    ///
    /// Returns an implementation-specific error if rendering or writing fails.
    fn render(&mut self, findings: &[Finding]) -> Result<(), Self::Error>;
}
