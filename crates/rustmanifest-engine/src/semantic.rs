// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tier-3 semantic analysis: workspace-level reasoning over the type graph.
//!
//! Unlike tiers 1 and 2, which inspect one file at a time, this tier builds
//! a [`TypeGraph`] across every analyzed source and evaluates checks that
//! need relationships between definitions: which structure aggregates which,
//! which field groups repeat, what a field's type refers to. Checks that
//! reason about modules rather than types build a module graph from the
//! same source slice — crate roots, `mod` resolution, and per-module `use`
//! targets.

mod analyzer;
mod behavior;
mod borrow_fix;
mod calls;
mod capability;
mod check;
mod composition;
mod coverage;
mod deduce;
mod doc_synth;
mod doc_truth;
mod docs;
mod emitted;
mod entry;
mod expr;
mod features;
mod field_group;
mod fingerprint;
mod graph;
mod handover;
mod implement;
mod insight;
mod intents;
mod invariant;
mod io_forms;
mod keeping;
mod kept;
mod knowledge;
mod layers;
mod layout;
mod loops;
mod meaning;
mod module_cycle;
pub(crate) mod module_graph;
mod module_overview;
mod montage;
mod naming;
mod oracle;
mod ownership;
mod rename_detect;
mod rename_op;
mod role;
mod scip;
mod settled;
#[cfg(any(test, feature = "fixtures"))]
pub mod settling;
mod std_derive;
mod std_knowledge;
mod summary;
mod surface;
mod whence;
mod why;
mod workspace;

pub use self::{
    analyzer::{SemanticAnalyzer, semantic_findings},
    behavior::{
        Acquisition, Alternative, Arguments, Arithmetic, Buffering, CHOICE_SET, CallSite, Channel,
        Choice, Closure, Comparison, Condition, Conduit, Conversion, Disposal, Effect, EffectSite,
        ExitRoute, ExitStatus, FunctionFacts, FunctionFlags, Grade, Guard, Guarding, HANDBACK_SET,
        Handback, Hole, HoleSource, INPUT_SOURCE_SET, InputRead, Intake, Junction, Literal,
        Multiplicity, Newline, OPERATION_SET, OUTPUT_SINK_SET, OpaqueReason, Operation,
        OperationSite, OriginGap, Outcome, OutputWrite, ParamFacts, ParamMode, ParamSlot, Payload,
        ProcessExit, Provenance, QuerySite, ReadShape, Segment, Selector, SelectorGap, SiteGuard,
        Statement, StatementCoverage, Stream, Terminator, VALUE_ORIGIN_SET, behavior,
        behavior_with, statement_coverage
    },
    calls::{
        CallGraph, RefusalRow, ResolutionContext, call_graph, call_graph_rows,
        call_graph_rows_with, call_graph_with, resolution_context, resolution_context_with,
        resolution_segments_with, transitive_effects
    },
    capability::{
        Capability, Evidence, ModuleCapability, ProvenCapability, Started, capabilities, started
    },
    coverage::{
        Coverage, coverage, coverage_indexed, coverage_with, origin_gaps, percent,
        unresolved_targets
    },
    deduce::{CatalogChoice, Deduction, deduce_impl},
    doc_synth::doc_draft,
    docs::{DocFacts, doc_facts},
    emitted::{Gain, install_impl},
    entry::{Completion, EntryKind, EntryPoint, entry_points},
    features::{Feature, features},
    fingerprint::behavior_fingerprint,
    graph::{
        Absorption, FieldNode, ImplNode, MethodNode, TraitNode, TypeGraph, TypeKind, TypeNode
    },
    handover::{Mark, copyness, handover_at, handover_marks},
    implement::impl_skeleton,
    intents::{DerivedIntent, derived_intents, derived_intents_with, fulfill_intents},
    invariant::{
        check_invariants, check_invariants_over, check_invariants_settled, check_invariants_with
    },
    keeping::{
        Command, Direction, Ending, Feeding, Keeping, Outliving, Serving, Store, TableStore,
        Telling, Verb, commands, endings, feedings, keeping, outliving, purpose, servings, stores,
        table_keeping, table_key, tables, telling
    },
    kept::why_kept,
    layers::{ModuleLayer, layers},
    layout::{TypeShape, shapes_with},
    meaning::{
        ChoiceClaim, Clause, ComputeClaim, EntryMeaning, ExitClaim, HandbackClaim, IntakeClaim,
        Narrative, OutputClaim, completion_rule, origin_text, program_meaning
    },
    module_overview::{ModuleEdge, ModuleInfo, ModuleOverview, module_overview},
    montage::{Montage, applied as montage_applied},
    oracle::{OracleImpl, OracleProvenance, TraitOracle},
    rename_detect::{Rename, detect_renames},
    rename_op::rename_module_fixes,
    role::{Role, role},
    scip::{ScipEdges, ScipError},
    std_derive::{StdFact, StdOutputKnowledge, output_knowledge, std_source_root},
    summary::{
        ComposedOrigin, Composition, FunctionSummary, ORIGIN_DEPTH, SUMMARY_SET, composition,
        composition_with, summaries, summaries_with
    },
    surface::{FrameworkModel, Route, Surface, surface},
    whence::{whence_at_over, whence_value, whence_value_over},
    why::{Answer, why_effect, why_effect_over},
    workspace::Workspace
};
