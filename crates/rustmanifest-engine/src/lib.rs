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

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

pub mod analyzer;
pub mod ast;
pub mod cache;
pub mod error;
pub mod facts_export;
pub mod orchestrator;
pub mod overview;
pub mod pattern;
pub mod pragma;
pub mod semantic;
pub mod source;
pub mod walker;

pub use analyzer::Analyzer;
pub use ast::AstAnalyzer;
pub use cache::FindingsCache;
pub use error::EngineError;
pub use orchestrator::{
    Cancellation, Orchestrator, OrchestratorBuilder, UNUSED_PRAGMA_RATIONALE_URI,
    UNUSED_PRAGMA_RULE_ID
};
pub use pattern::PatternAnalyzer;
pub use semantic::{
    Absorption, CallGraph, Coverage, DocFacts, Effect, ExpansionReport, Feature, FunctionFacts,
    FunctionFlags, ImplNode, MethodNode, ModuleEdge, ModuleInfo, ModuleOverview, ParamFacts,
    ParamMode, Role, Route, ScipEdges, ScipError, SemanticAnalyzer, Surface, TraitNode, TypeGraph,
    TypeKind, TypeNode, behavior, call_graph, call_graph_with, check_invariants,
    check_invariants_with, coverage, coverage_indexed, coverage_with, doc_facts, expansion,
    features, module_overview, role, surface, transitive_effects, unresolved_targets
};
pub use source::Source;
