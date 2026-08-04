// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The library's own spellings of an arithmetic operation, with what each
//! does about the failure that operation can have.
//!
//! A method absent from this list is not read as an operation. A namesake
//! of the program's own would otherwise be credited with the library's
//! behaviour, which is the standing assumption every name-keyed
//! recognizer here refuses to make quietly.
//!
//! The plain spellings — the operators themselves — are deliberately
//! absent: they are syntax, and the reading that owns them is the one
//! that walks the operator.

use crate::semantic::behavior::{Arithmetic, Guarding, Operation};

/// Library methods performing one arithmetic operation, with what each
/// does about its failure.
const OPERATION_METHODS: [(&str, Arithmetic, Guarding); 20] = [
    ("checked_add", Arithmetic::Add, Guarding::Checked),
    ("checked_div", Arithmetic::Divide, Guarding::Checked),
    ("checked_mul", Arithmetic::Multiply, Guarding::Checked),
    ("checked_neg", Arithmetic::Negate, Guarding::Checked),
    ("checked_rem", Arithmetic::Remainder, Guarding::Checked),
    ("checked_sub", Arithmetic::Subtract, Guarding::Checked),
    ("overflowing_add", Arithmetic::Add, Guarding::Reported),
    ("overflowing_mul", Arithmetic::Multiply, Guarding::Reported),
    ("overflowing_sub", Arithmetic::Subtract, Guarding::Reported),
    ("saturating_add", Arithmetic::Add, Guarding::Saturating),
    ("saturating_mul", Arithmetic::Multiply, Guarding::Saturating),
    ("saturating_sub", Arithmetic::Subtract, Guarding::Saturating),
    ("wrapping_add", Arithmetic::Add, Guarding::Wrapping),
    ("wrapping_div", Arithmetic::Divide, Guarding::Wrapping),
    ("wrapping_mul", Arithmetic::Multiply, Guarding::Wrapping),
    ("wrapping_neg", Arithmetic::Negate, Guarding::Wrapping),
    ("wrapping_rem", Arithmetic::Remainder, Guarding::Wrapping),
    ("wrapping_sub", Arithmetic::Subtract, Guarding::Wrapping),
    ("abs_diff", Arithmetic::Subtract, Guarding::Saturating),
    ("pow", Arithmetic::Multiply, Guarding::Unchecked)
];

/// The operation a library method performs, with what it does about the
/// failure, where a row names one.
pub(in crate::semantic) fn operation_method(method: &str) -> Option<(Operation, Guarding)> {
    OPERATION_METHODS
        .iter()
        .find(|(named, _, _)| *named == method)
        .map(|(_, arithmetic, guarding)| (Operation::Arithmetic(*arithmetic), *guarding))
}
