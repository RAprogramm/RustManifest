// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the standard library's own calls hand back, and whether they can
//! fail.
//!
//! Rows, not branches: the reading that consults them knows nothing about
//! any particular method, and a method absent from the list is absent
//! from the answer rather than guessed at.

use crate::semantic::knowledge::{MethodResult, Outcome, ReturnForm};

/// What each known standard-library method hands back.
const METHOD_RESULTS: [MethodResult; 8] = [
    MethodResult {
        owner:   "str",
        method:  "parse",
        form:    ReturnForm::TypeArgument(0),
        outcome: Outcome::Fallible
    },
    MethodResult {
        owner:   "String",
        method:  "parse",
        form:    ReturnForm::TypeArgument(0),
        outcome: Outcome::Fallible
    },
    MethodResult {
        owner:   "String",
        method:  "as_str",
        form:    ReturnForm::Named("str"),
        outcome: Outcome::Total
    },
    MethodResult {
        owner:   "Args",
        method:  "next",
        form:    ReturnForm::Named("String"),
        outcome: Outcome::Optional
    },
    MethodResult {
        owner:   "Args",
        method:  "nth",
        form:    ReturnForm::Named("String"),
        outcome: Outcome::Optional
    },
    MethodResult {
        owner:   "Vec",
        method:  "get",
        form:    ReturnForm::Element,
        outcome: Outcome::Optional
    },
    MethodResult {
        owner:   "str",
        method:  "trim",
        form:    ReturnForm::Named("str"),
        outcome: Outcome::Total
    },
    MethodResult {
        owner:   "String",
        method:  "trim",
        form:    ReturnForm::Named("str"),
        outcome: Outcome::Total
    }
];

/// What a known standard-library method hands back, where a row names it.
pub(in crate::semantic) fn method_result(owner: &str, method: &str) -> Option<MethodResult> {
    METHOD_RESULTS
        .iter()
        .find(|row| row.owner == owner && row.method == method)
        .copied()
}
