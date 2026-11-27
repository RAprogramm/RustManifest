// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program computes: one record per operation over values.
//!
//! The call channel says a method was called and the effect list says
//! nothing at all about arithmetic. Neither can say that two values were
//! added, which two, or what the sum was written as. This channel can.
//!
//! Built on the terms every per-site channel here is built on: one record
//! per construct, in body order, never deduplicated, each carrying what
//! stands over it. Operands are named by origin, so an operation over a
//! value drawn from outside the process says so without a second reading.
//!
//! Two things are deliberately not claimed. What a bare arithmetic
//! operator does on failure is fixed by the build rather than by the
//! source, so the guarding of one is recorded as unchecked and the claim
//! says the build decides; and a type the source never writes is left
//! unidentified rather than inferred from a later use.

mod guarding;
mod kind;
mod site;

pub use self::{
    guarding::Guarding,
    kind::{Arithmetic, Comparison, Conversion, Junction, Operation},
    site::OperationSite
};

/// Version of the operation shapes this channel recognizes. Stamped into
/// claims so a negative result states which recognizer produced it.
pub const OPERATION_SET: u32 = 1;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Arithmetic, Comparison, Guarding, Operation, OperationSite};
    use crate::{
        semantic::behavior::{Literal, Provenance},
        source::Source
    };

    fn computed(body: &str) -> Vec<OperationSite> {
        let text = format!("fn compute(left: u32, right: u32) -> u32 {{\n{body}\n}}");
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text)];
        crate::semantic::behavior_with(
            &sources,
            Some(&crate::semantic::settling::settled_over(&sources))
        )
        .iter()
        .find(|fact| fact.function == "compute")
        .map(|fact| fact.operations.clone())
        .expect("the function is a fact")
    }

    /// An operator computes over two values, and each operand is named by
    /// where it came from — which is what makes the record more than a
    /// note that arithmetic happened.
    #[test]
    fn an_operator_names_its_operation_and_both_operands() {
        let recorded = computed("    left + 1");
        assert_eq!(recorded.len(), 1);
        assert_eq!(
            recorded[0].operation,
            Operation::Arithmetic(Arithmetic::Add)
        );
        let [first, second] = recorded[0].operands.as_slice() else {
            panic!("two operands: {:?}", recorded[0].operands);
        };
        assert!(matches!(first, Provenance::Parameter(slot) if slot.position == 0));
        assert_eq!(
            second,
            &Provenance::Written(Literal::Integer("1".to_owned()))
        );
    }

    /// A comparison is an operation of its own: it answers with a truth
    /// value rather than a number, and a claim about one is never a claim
    /// about the other.
    #[test]
    fn a_comparison_is_recorded_as_a_comparison() {
        let recorded = computed("    if right == 0 { return 0; }\n    left / right");
        let kinds: Vec<&Operation> = recorded.iter().map(|site| &site.operation).collect();
        assert!(kinds.contains(&&Operation::Comparison(Comparison::Equal)));
        assert!(kinds.contains(&&Operation::Arithmetic(Arithmetic::Divide)));
    }

    /// What the library does about a failure is part of the operation, so
    /// two spellings of one addition are told apart.
    #[test]
    fn the_library_spellings_carry_what_they_do_about_failure() {
        assert_eq!(
            computed("    left.checked_add(right).unwrap_or(0)")[0].guarding,
            Guarding::Checked
        );
        assert_eq!(
            computed("    left.saturating_add(right)")[0].guarding,
            Guarding::Saturating
        );
        assert_eq!(
            computed("    left + right")[0].guarding,
            Guarding::Unchecked
        );
    }

    /// A written cast names the type it produces, because the source
    /// writes it; a type written nowhere stays unidentified.
    #[test]
    fn a_written_cast_names_the_type_it_produces() {
        let recorded = computed("    (left as u8) as u32");
        assert_eq!(recorded[0].produced.as_deref(), Some("u8"));
    }

    /// An element taken at a position is an operation over two values,
    /// the sequence and the position.
    #[test]
    fn an_element_taken_at_a_position_is_an_operation() {
        let recorded = computed("    let values = [1, 2];\n    values[1]");
        assert!(
            recorded
                .iter()
                .any(|site| site.operation == Operation::Selection)
        );
    }

    /// A body repeating its work says so, on the same terms a repeated
    /// write does.
    #[test]
    fn an_operation_inside_a_loop_states_that_it_repeats() {
        let recorded = computed("    for _ in 0..3 { let _ = left + right; }\n    0");
        assert!(
            recorded.iter().any(
                |site| site.multiplicity == crate::semantic::behavior::Multiplicity::Unbounded
            )
        );
    }
}
