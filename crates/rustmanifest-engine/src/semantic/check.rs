// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The catalog of built-in semantic checks and the dispatch from a rules
//! pack identifier to the module that evaluates it.
//!
//! Lives in its own module so that the tier root carries only module
//! declarations, re-exports and the analyzer entry point, while every
//! addition of a check touches exactly one place: the enumeration below.

use super::{
    composition, dead, doc_truth, field_group, insight::Insight, io_forms, loops, module_cycle,
    naming, orphan, ownership, workspace::Workspace
};

/// Built-in semantic checks addressable from `definition.semantic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Check {
    /// `repeated-field-group` — identical field groups across structures.
    RepeatedFieldGroup,
    /// `module-cycles` — cycles in the module dependency graph.
    ModuleCycles,
    /// `structure-size` — oversized structures with a graph-derived split
    /// suggestion.
    StructureSize,
    /// `borrowable-params` — owned non-`Copy` parameters that only read
    /// fields and should be shared borrows.
    BorrowableParams,
    /// `loop-invariant-clone` — clones of loop-invariant places that are
    /// only read and could be hoisted out of the loop.
    LoopInvariantClone,
    /// `loop-allocation` — fresh container allocations repeated every
    /// iteration although the value never escapes the iteration.
    LoopAllocation,
    /// `deceptive-name` — read-named functions that write state or touch
    /// the outside world.
    DeceptiveName,
    /// `deceptive-doc` — doc summaries claiming a read over a mutating
    /// body.
    DeceptiveDoc,
    /// `orphan-file` — files reachable from no crate root, with a
    /// plausible forgotten `mod` mount point.
    OrphanFile,
    /// `dead-function` — private functions no call ever mentions.
    DeadFunction,
    /// `output-lock-run` — a run of prints taking the stream lock once
    /// per call.
    OutputLockRun,
    /// `output-in-loop` — a write inside a loop over a line-buffered
    /// stream.
    OutputInLoop,
    /// `output-constant-format` — a proven constant carried through the
    /// formatting machinery.
    OutputConstantFormat,
    /// `output-error-lost` — an entry point whose failed write panics
    /// instead of leaving as a returned error.
    OutputErrorLost
}

impl Check {
    /// Resolves a check identifier from the rules pack.
    pub(super) fn resolved(name: &str) -> Option<Self> {
        match name {
            "repeated-field-group" => Some(Self::RepeatedFieldGroup),
            "module-cycles" => Some(Self::ModuleCycles),
            "structure-size" => Some(Self::StructureSize),
            "borrowable-params" => Some(Self::BorrowableParams),
            "loop-invariant-clone" => Some(Self::LoopInvariantClone),
            "loop-allocation" => Some(Self::LoopAllocation),
            "deceptive-name" => Some(Self::DeceptiveName),
            "deceptive-doc" => Some(Self::DeceptiveDoc),
            "orphan-file" => Some(Self::OrphanFile),
            "dead-function" => Some(Self::DeadFunction),
            "output-lock-run" => Some(Self::OutputLockRun),
            "output-in-loop" => Some(Self::OutputInLoop),
            "output-constant-format" => Some(Self::OutputConstantFormat),
            "output-error-lost" => Some(Self::OutputErrorLost),
            _ => None
        }
    }

    /// Runs the check over the analyzed workspace, reading only the
    /// derivations it needs — an unread derivation is never taken.
    pub(super) fn insights(self, workspace: &Workspace<'_>) -> Vec<Insight> {
        match self {
            Self::RepeatedFieldGroup => field_group::insights(workspace),
            Self::ModuleCycles => module_cycle::insights(workspace),
            Self::StructureSize => composition::insights(workspace.types()),
            Self::BorrowableParams => ownership::insights(workspace),
            Self::LoopInvariantClone => loops::clone_insights(workspace),
            Self::LoopAllocation => loops::allocation_insights(workspace),
            Self::DeceptiveName => naming::insights(workspace),
            Self::DeceptiveDoc => doc_truth::insights(workspace),
            Self::OrphanFile => orphan::insights(workspace),
            Self::DeadFunction => dead::insights(workspace),
            Self::OutputLockRun => io_forms::lock_run_insights(workspace),
            Self::OutputInLoop => io_forms::loop_write_insights(workspace),
            Self::OutputConstantFormat => io_forms::literal_format_insights(workspace),
            Self::OutputErrorLost => io_forms::lost_write_error_insights(workspace)
        }
    }
}
