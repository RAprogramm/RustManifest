// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Object-rooted answers of the tools.
//!
//! The protocol requires every structured tool result to be an object
//! at its root; a bare array there fails the client's schema check
//! before a single call is made. Every list a tool hands back rides in
//! a named field of one of these envelopes.

use rustmanifest_schema::Finding;

/// The findings of one review run.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(super) struct FindingsAnswer {
    /// Findings in engine order.
    pub(super) findings: Vec<Finding>
}

/// The behavioral facts of one path.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(super) struct FactsAnswer {
    /// Facts as the engine exports them.
    pub(super) facts: serde_json::Value
}

/// The learned convention clauses of one path.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(super) struct ClausesAnswer {
    /// Clauses in training order.
    pub(super) clauses: Vec<crate::knowledge::ConventionClause>
}

/// The inline suppressions of one path.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(super) struct SuppressionsAnswer {
    /// Suppressions in file-then-line order.
    pub(super) suppressions: Vec<crate::knowledge::SuppressionRecord>
}
