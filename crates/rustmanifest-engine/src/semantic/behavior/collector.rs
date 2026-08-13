// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Body analysis: the syntax visitor that accumulates behavioral facts —
//! reads, writes, constructions, effects, clones, and allocations — from
//! one function body against its signature-derived parameter types.
//!
//! The state the walk carries lives here; every rule that reads a node
//! and every table lookup that answers for one sits in a submodule beside
//! this file, so the accumulated state and the readings over it stay
//! separable.

mod assignment;
mod binding;
mod block;
mod bound;
mod branch;
mod branch_flow;
mod call;
mod call_type;
mod census;

mod catalog;
mod choice;
mod completion;
mod construction;
mod deferral;
mod destination;
mod divergence;
mod effect;
mod expansion;
mod expression_flow;
mod field_fact;
mod flow;
mod gate;
mod handback;
mod input_read;
mod intake_chain;
mod invocation;
mod jumps;
mod multiplicity;
mod naming;
mod operation;
mod origin;
mod outcome;
mod output_write;
mod pattern;
mod process_exit;
mod repetition;
mod resolution;
mod scope;
mod site;
mod statement_flow;
mod typing;
mod unwrapping;
mod visit;
mod wrapper;

use std::collections::{BTreeMap, BTreeSet};

pub(super) use self::census::name_census;
use super::{
    output::Guard,
    types::FunctionFacts,
    value::{Condition, Provenance}
};

/// Syntax visitor accumulating facts from one function body.
pub(super) struct FactCollector {
    /// Facts collected so far.
    pub(super) facts:            FunctionFacts,
    /// Parameter name to type-name mapping from the signature.
    pub(super) param_types:      BTreeMap<String, String>,
    /// What a resolver settled about this source, by byte range: the type
    /// the expression at that position hands back. Empty where no
    /// resolver ran.
    pub(super) settled:          std::collections::BTreeMap<(u32, u32), String>,
    /// The type a resolver settled for the receiver of the call at each
    /// byte range. Empty where no resolver ran.
    pub(super) receivers:        std::collections::BTreeMap<(u32, u32), String>,
    /// The type holding the field read at each byte range, and the type
    /// that field itself has. Empty where no resolver ran.
    pub(super) held:             std::collections::BTreeMap<(u32, u32), (String, String)>,
    /// The type a resolver settled for the expression at each byte range,
    /// whatever shape it was written in. Empty where no resolver ran.
    pub(super) typed:            std::collections::BTreeMap<(u32, u32), String>,
    /// The type a resolver settled for the expression at each byte range,
    /// as the compiler renders it in full. Empty where no resolver ran.
    pub(super) rendered:         std::collections::BTreeMap<(u32, u32), String>,
    /// How the call at each byte range takes the value it is written on.
    pub(super) takes:            std::collections::BTreeMap<(u32, u32), super::types::ParamMode>,
    /// The full path a resolver settled the path at each byte range to
    /// reach. Empty where no resolver ran.
    pub(super) named:            std::collections::BTreeMap<(u32, u32), String>,
    /// The kind of item a resolver settled the path at each byte range to
    /// reach. Empty where no resolver ran.
    pub(super) items:            std::collections::BTreeMap<(u32, u32), String>,
    /// Let bindings holding a wrapper value with a known wrapped inner
    /// type, cloned or borrowed off a wrapper field.
    pub(super) local_wraps:      BTreeMap<String, String>,
    /// Imported names of the enclosing file to their full `use` paths.
    pub(super) imports:          BTreeMap<String, String>,
    /// Let bindings holding a value provably constructed in this body.
    pub(super) fresh:            BTreeSet<String>,
    /// Let bindings holding a written buffering wrapper over one of the
    /// process's own streams, by the stream the wrapper fronts.
    pub(super) buffered_streams: BTreeMap<String, crate::semantic::behavior::output::Stream>,
    /// Constructs currently enclosing the visited node, outermost first.
    pub(super) guards:           Vec<Guard>,
    /// Exits that may leave the function before the visited node.
    pub(super) prior_exits:      u32,
    /// Whether the visited node follows an unconditional divergence.
    pub(super) dead:             bool,
    /// One entry per declared input of the signature, the receiver first.
    pub(super) slots:            Vec<Option<String>>,
    /// Where each name this body gave a value to got it.
    pub(super) origins:          BTreeMap<String, Provenance>,
    /// Byte ranges whose settled type is the never type.
    pub(super) leaving:          BTreeSet<(u32, u32)>,
    /// Names this body writes at all, by binding or by assignment.
    pub(super) written:          BTreeSet<String>,
    /// Names this body writes exactly once, so a reading of one resolves
    /// to a single value.
    pub(super) sole:             BTreeSet<String>,
    /// Origins produced by records the walk has pushed, by the byte range
    /// of the construct that produced them.
    pub(super) produced:         BTreeMap<(u32, u32), Provenance>,
    /// What each enclosing construct tests, outermost first.
    pub(super) conditions:       Vec<Condition>
}
