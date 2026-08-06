// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tiered analysis engine for `rustmanifest`.
//!
//! Rules are checked at one of three tiers, chosen per rule: a
//! [`PatternAnalyzer`] scanning file text, an [`AstAnalyzer`] walking the
//! `syn` tree of one file, and the [`semantic`] tier reading the whole
//! workspace. An [`Orchestrator`] runs them in parallel over the files
//! [`walker::discover`] finds, emits byte-precise findings, and honours
//! the [`pragma`] suppressions written in the source.
//!
//! Above the rules sits what the engine *derives* rather than checks, all
//! of it in [`semantic`] and none of it requiring a compiler: the type and
//! module graphs, per-function behavioral facts, the call graph, the
//! external surface a service exposes, what each subsystem can do to the
//! world, what a program writes out and reads in, and where a value came
//! from — across calls, through the per-function summary. Every one of
//! those answers is a record with the site that proves it, and every
//! position the reading could not settle is a named refusal rather than a
//! silence, counted by [`coverage`] so a reader can tell the share that
//! was answered from the share that was merely recorded.
//!
//! What a resolver settles about a workspace — which function a call
//! reaches, what it hands back — arrives as a plain-data overlay from the
//! `rustmanifest-resolve` binary; nothing here links a compiler frontend.

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
    Absorption, Arguments, CallGraph, Capability, ChoiceClaim, Clause, Completion, ComputeClaim,
    Condition, Coverage, Disposal, DocFacts, Effect, EntryKind, EntryMeaning, EntryPoint,
    Evidence, ExitClaim, ExitRoute, ExitStatus, ExpansionReport, Feature, FunctionFacts,
    FunctionFlags, Grade, Guard, HandbackClaim, Hole, HoleSource, ImplNode, Literal, MethodNode,
    ModuleCapability, ModuleEdge, ModuleInfo, ModuleOverview, Multiplicity, Narrative, Newline,
    OUTPUT_SINK_SET, OpaqueReason, OriginGap, OutputClaim, OutputWrite, ParamFacts, ParamMode,
    ParamSlot, Payload, ProcessExit, Provenance, Role, Route, ScipEdges, ScipError, Segment,
    SemanticAnalyzer, SiteGuard, Started, Stream, Surface, TraitNode, TypeGraph, TypeKind,
    TypeNode, VALUE_ORIGIN_SET, Workspace, behavior, behavior_with, call_graph, call_graph_with,
    capabilities, check_invariants, check_invariants_with, completion_rule, coverage,
    coverage_indexed, coverage_with, doc_facts, entry_points, expansion, features,
    module_overview, origin_gaps, program_meaning, role, semantic_findings, started, surface,
    transitive_effects, unresolved_targets
};
pub use source::Source;

/// Everything the engine derived about the print path of the active
/// toolchain, each fact carrying the library location that proves it.
///
/// Returns an empty list when the toolchain ships no library sources, so
/// a caller states that nothing was derived rather than filling the gap.
#[must_use]
pub fn library_output_facts() -> Vec<semantic::StdFact> {
    semantic::std_source_root()
        .map_or_else(Vec::new, |root| semantic::output_knowledge(&root).facts)
}
