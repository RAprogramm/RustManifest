// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where a value came from.
//!
//! The call channel answers "something was called" and cannot answer
//! more: a candidate string with no operands, no result, and no link to
//! the value the call produced. This module holds the term that can.
//!
//! A value is a thing the source writes out, a thing that arrived in one
//! of this function's own inputs, a thing drawn from outside the process,
//! a thing computed from other values, a thing one of several
//! alternatives produces, or a thing a call handed back. Every question
//! the reading cannot settle is a leaf naming the question, never a leaf
//! standing in for an answer.
//!
//! A reference inside one body is an index into a body-order channel of
//! the same function, never an inlined subtree: two consumers of one
//! value name the same record instead of carrying two copies of it.

mod argument;
mod condition;
mod disposal;
mod gap;
mod literal;
mod provenance;
mod slot;

pub use self::{
    argument::Arguments, condition::Condition, disposal::Disposal, gap::OriginGap,
    literal::Literal, provenance::Provenance, slot::ParamSlot
};

/// Version of the expression shapes the origin reading recognizes.
///
/// Stamped into every claim carrying an origin, so a refusal states which
/// reading produced it. Hand-maintained exactly as the write channel's
/// sink set is: any change to the recognized shapes obliges a bump.
pub const VALUE_ORIGIN_SET: u32 = 1;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Arguments, Literal, OriginGap, Provenance};
    use crate::{semantic::behavior::behavior, source::Source};

    /// The arguments of the last call site of one function, in written
    /// order.
    fn handed_over(text: &str, function: &str) -> Arguments {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)))
            .iter()
            .find(|fact| fact.function == function)
            .and_then(|fact| fact.call_sites.last().map(|site| site.arguments.clone()))
            .expect("the function records a call site")
    }

    /// A written literal and one of the function's own inputs are the two
    /// leaves of the term, and a call site names both by position.
    #[test]
    fn a_call_site_names_the_origin_of_every_argument_it_writes() {
        let handed = handed_over("fn caller(amount: u32) { helper(amount, 1); }", "caller");
        let written = handed.written().expect("the arguments are written");
        assert_eq!(written.len(), 2);
        let Provenance::Parameter(slot) = &written[0] else {
            panic!("the first argument is this function's own input: {written:?}");
        };
        assert_eq!(slot.position, 0);
        assert_eq!(slot.name.as_deref(), Some("amount"));
        assert_eq!(
            written[1],
            Provenance::Written(Literal::Integer("1".to_owned()))
        );
    }

    /// A name this body writes twice stands for no single value, so a
    /// reading of it refuses by name instead of picking one.
    #[test]
    fn a_name_written_twice_refuses_rather_than_choosing() {
        let handed = handed_over("fn caller() { let v = 1; let v = 2; helper(v); }", "caller");
        assert_eq!(
            handed.written().expect("the arguments are written"),
            [Provenance::NotDerived(OriginGap::Rebound)]
        );
    }

    /// A site whose arguments the source writes nowhere says so: an empty
    /// list would read as a call of no arguments, which is a different
    /// fact.
    #[test]
    fn a_site_written_as_no_call_states_that_its_arguments_are_unwritten() {
        let text = "fn caller() -> Result<u8, Outer> { inner()? }\nfn inner() -> Result<u8, \
                    Inner> { Ok(1) }";
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = crate::semantic::settling::Settling::over("src/lib.rs", text)
            .typed("inner()", "Result<u8, Inner>")
            .done();
        let facts = crate::semantic::behavior_with(&sources, Some(&resolution));
        let caller = facts
            .iter()
            .find(|fact| fact.function == "caller")
            .expect("the caller is a fact");
        let conversion = caller
            .call_sites
            .iter()
            .find(|site| site.candidate == "Outer::from")
            .expect("the error conversion is recorded");
        assert_eq!(conversion.arguments, Arguments::Unwritten);
        assert_eq!(conversion.site.start, 0);
    }

    /// The value a binding holds is the value its initializer produced, so
    /// a later reading of the name reaches through it.
    #[test]
    fn a_binding_hands_on_the_origin_of_its_initializer() {
        let handed = handed_over("fn caller() { let value = 7; helper(value); }", "caller");
        assert_eq!(
            handed.written().expect("the arguments are written"),
            [Provenance::Written(Literal::Integer("7".to_owned()))]
        );
    }
}
