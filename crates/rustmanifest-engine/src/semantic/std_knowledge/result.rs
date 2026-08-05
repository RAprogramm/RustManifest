// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the standard library's own calls hand back, and whether they can
//! fail.
//!
//! Rows, not branches: the reading that consults them knows nothing about
//! any particular method, and a method absent from the list is absent
//! from the answer rather than guessed at.
//!
//! A row states only what the library's own documentation guarantees. That
//! `Option::is_some` hands back a `bool` and cannot fail is such a
//! guarantee; what the `bool` *means* about the receiver is not one this
//! row makes, and the reading does not claim it.

use crate::semantic::knowledge::{MethodResult, Outcome, ReturnForm};

/// What each known standard-library method hands back.
const METHOD_RESULTS: [MethodResult; 16] = [
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
    },
    MethodResult {
        owner:   "Option",
        method:  "is_some",
        form:    ReturnForm::Named("bool"),
        outcome: Outcome::Total
    },
    MethodResult {
        owner:   "Option",
        method:  "is_none",
        form:    ReturnForm::Named("bool"),
        outcome: Outcome::Total
    },
    MethodResult {
        owner:   "Result",
        method:  "is_ok",
        form:    ReturnForm::Named("bool"),
        outcome: Outcome::Total
    },
    MethodResult {
        owner:   "Result",
        method:  "is_err",
        form:    ReturnForm::Named("bool"),
        outcome: Outcome::Total
    },
    MethodResult {
        owner:   "BTreeMap",
        method:  "get",
        form:    ReturnForm::Element,
        outcome: Outcome::Optional
    },
    MethodResult {
        owner:   "HashMap",
        method:  "get",
        form:    ReturnForm::Element,
        outcome: Outcome::Optional
    },
    MethodResult {
        owner:   "BTreeMap",
        method:  "remove",
        form:    ReturnForm::Element,
        outcome: Outcome::Optional
    },
    MethodResult {
        owner:   "HashMap",
        method:  "remove",
        form:    ReturnForm::Element,
        outcome: Outcome::Optional
    }
];

/// What a known standard-library method hands back, where a row names it.
pub(in crate::semantic) fn method_result(owner: &str, method: &str) -> Option<MethodResult> {
    METHOD_RESULTS
        .iter()
        .find(|row| row.owner == owner && row.method == method)
        .copied()
}
