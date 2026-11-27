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

/// The bodies of the numeric module performing one arithmetic
/// operation, each named by the item the call reaches, with what each
/// does about its failure.
///
/// Twenty written method names keyed this before, and keyed it for any
/// workspace method spelled the same way: the compiler settles which
/// body a call reaches, and only the library's own carries the
/// guarantee stated here.
const OPERATION_METHODS: [(&str, Arithmetic, Guarding); 20] = [
    ("core::num::checked_add", Arithmetic::Add, Guarding::Checked),
    (
        "core::num::checked_div",
        Arithmetic::Divide,
        Guarding::Checked
    ),
    (
        "core::num::checked_mul",
        Arithmetic::Multiply,
        Guarding::Checked
    ),
    (
        "core::num::checked_neg",
        Arithmetic::Negate,
        Guarding::Checked
    ),
    (
        "core::num::checked_rem",
        Arithmetic::Remainder,
        Guarding::Checked
    ),
    (
        "core::num::checked_sub",
        Arithmetic::Subtract,
        Guarding::Checked
    ),
    (
        "core::num::overflowing_add",
        Arithmetic::Add,
        Guarding::Reported
    ),
    (
        "core::num::overflowing_mul",
        Arithmetic::Multiply,
        Guarding::Reported
    ),
    (
        "core::num::overflowing_sub",
        Arithmetic::Subtract,
        Guarding::Reported
    ),
    (
        "core::num::saturating_add",
        Arithmetic::Add,
        Guarding::Saturating
    ),
    (
        "core::num::saturating_mul",
        Arithmetic::Multiply,
        Guarding::Saturating
    ),
    (
        "core::num::saturating_sub",
        Arithmetic::Subtract,
        Guarding::Saturating
    ),
    (
        "core::num::wrapping_add",
        Arithmetic::Add,
        Guarding::Wrapping
    ),
    (
        "core::num::wrapping_div",
        Arithmetic::Divide,
        Guarding::Wrapping
    ),
    (
        "core::num::wrapping_mul",
        Arithmetic::Multiply,
        Guarding::Wrapping
    ),
    (
        "core::num::wrapping_neg",
        Arithmetic::Negate,
        Guarding::Wrapping
    ),
    (
        "core::num::wrapping_rem",
        Arithmetic::Remainder,
        Guarding::Wrapping
    ),
    (
        "core::num::wrapping_sub",
        Arithmetic::Subtract,
        Guarding::Wrapping
    ),
    (
        "core::num::abs_diff",
        Arithmetic::Subtract,
        Guarding::Saturating
    ),
    ("core::num::pow", Arithmetic::Multiply, Guarding::Unchecked)
];

/// The operation the body a call reaches performs, with what it does
/// about the failure, where a row names it.
pub(in crate::semantic) fn operation_method(reached: &str) -> Option<(Operation, Guarding)> {
    OPERATION_METHODS
        .iter()
        .find(|(named, _, _)| *named == reached)
        .map(|(_, arithmetic, guarding)| (Operation::Arithmetic(*arithmetic), *guarding))
}
