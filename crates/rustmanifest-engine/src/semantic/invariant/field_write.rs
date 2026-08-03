// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The field-writer constraint: who may assign a named field.
//!
//! An invariant carrying `field` plus `write-only-from` states that the
//! observed write must originate from one of the listed modules. The
//! violation is reported against the function whose body performs the
//! write, with the derivation naming the write, the module it lives in,
//! and the allowlist it falls outside of.

use rustmanifest_schema::{Invariant, TraceStep};

use super::scope::covered_by_any;
use crate::semantic::behavior::FunctionFacts;

/// Checks the field-writer constraint of one invariant against one
/// function's facts.
pub(super) fn field_violation(
    invariant: &Invariant,
    fact: &FunctionFacts,
    module: &str
) -> Option<(String, Vec<TraceStep>)> {
    let field = invariant.field.as_deref()?;
    if !fact.writes.iter().any(|written| written == field) {
        return None;
    }
    if covered_by_any(module, &invariant.write_only_from) {
        return None;
    }
    let allowed = invariant.write_only_from.join(", ");
    let detail = format!(
        "invariant violated: `{}` writes `{field}` from module `{module}`, but only [{allowed}] \
         may write it",
        fact.function
    );
    let trace = vec![
        TraceStep {
            fact:   format!("fn `{}` writes `{field}`", fact.function),
            reason: "observed assignment or mutating call in the body".to_owned()
        },
        TraceStep {
            fact:   format!("fn `{}` lives in module `{module}`", fact.function),
            reason: "module graph maps its file to this path".to_owned()
        },
        TraceStep {
            fact:   format!("allowed writers: [{allowed}]"),
            reason: "module is not covered by any allowed prefix".to_owned()
        },
    ];
    Some((detail, trace))
}
