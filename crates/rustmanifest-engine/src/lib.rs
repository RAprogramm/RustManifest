// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tiered analysis engine for `rustmanifest`.
//!
//! The engine exposes three tiers selectable per-rule:
//!
//! - **Pattern** — fast regex / aho-corasick scan over file text.
//! - **AST** — `syn`-based traversal for structural and local semantic checks.
//! - **Semantic** — full semantic analysis via `rust-analyzer` or cargo
//!   integrations (off by default).
//!
//! Phase 1B implements the **pattern** tier end-to-end: a [`PatternAnalyzer`]
//! per rule, an [`Orchestrator`] coordinating parallel execution across files
//! discovered through [`walker::discover`], plus byte-precise [`Finding`]s
//! and a [`pragma`] suppression mechanism.

pub mod analyzer;
pub mod error;
pub mod orchestrator;
pub mod pattern;
pub mod pragma;
pub mod source;
pub mod walker;

pub use analyzer::Analyzer;
pub use error::EngineError;
pub use orchestrator::{Cancellation, Orchestrator, OrchestratorBuilder};
pub use pattern::PatternAnalyzer;
pub use source::Source;
