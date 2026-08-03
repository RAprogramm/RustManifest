// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Resolved-name accuracy layer: call edges read from a SCIP index.
//!
//! `rust-analyzer scip .` emits a protobuf index whose occurrences carry
//! name-resolved symbols with definition ranges. This module turns that
//! index into per-caller callee lists the call graph can consume as an
//! optional overlay: a reference occurrence of a function symbol is
//! attributed to the smallest function definition whose enclosing range
//! contains it. The layer is opt-in — the index is produced by an
//! external tool the user runs, never by the engine itself — and purely
//! additive: it can only add edges the syntactic resolver missed, never
//! remove or reclassify one.
//!
//! Documented limits: the index reflects one cfg universe (rust-analyzer
//! indexes active code only, so feature-gated bodies need an index built
//! with those features on), and operator desugarings recorded by
//! rust-analyzer as trait calls (`?`, `.await`, arithmetic) are kept out
//! by the same function-symbol filter that admits real calls.

mod definition;
mod edges;
mod identity;
mod symbol;
mod wire;

pub use edges::{ScipEdges, ScipError};
