// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where one value came from, as one body proves it.

use super::{gap::OriginGap, literal::Literal, slot::ParamSlot};

/// Where one value came from, as one body proves it.
///
/// Six positive answers and one refusal. A written literal and an input
/// of this function are leaves: neither is an event, so neither has a
/// site. The other four name a record of the owning function, by channel
/// and by position in it — an index, never an inlined subtree, so two
/// consumers of one value name the same record instead of carrying two
/// copies of it. Unfolding those indices into a tree is the composition
/// tier's work and is bounded there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// A value written in the source.
    Written(Literal),
    /// The value one of this function's own inputs arrived as. The one
    /// free variable of the term, and the whole of why a summary
    /// composes: a caller substitutes the origin of its own argument for
    /// this slot and is left with a term naming nothing of the callee's.
    Parameter(ParamSlot),
    /// The value the read at this position of the input channel draws.
    Read(u32),
    /// The value the operation at this position of the operation channel
    /// produces.
    Computed(u32),
    /// Whichever value the alternative taken by the choice at this
    /// position of the choice channel produces.
    Chosen(u32),
    /// The value the call at this position of the call-site channel hands
    /// back.
    Called(u32),
    /// The value a field of a known type holds, named `Owner.field`.
    ///
    /// What put it there is decided wherever that type is constructed,
    /// which is outside this body; that it is this field of this type is
    /// what the body proves, and it is a fact rather than the absence of
    /// one. A field whose owning type the tables cannot name is not this
    /// answer — it is the refusal that says so.
    Field(String),
    /// The value a path of the standard library stands for, named as
    /// written: a unit variant like `None`, or an associated constant
    /// like `u32::MAX`.
    ///
    /// Not a name this body bound and not a refusal: what the path holds
    /// is fixed by the library, so the position is as settled as a
    /// written literal. Only paths the library tables answer for land
    /// here — a workspace constant of the same spelling does not.
    Constant(String),
    /// No value: control leaves this position instead of producing one.
    ///
    /// A settled answer and not a refusal. `Err(_) => return code` hands
    /// nothing back to the choice standing over it, and the language
    /// guarantees that — reading it as a value whose origin failed would
    /// call a certainty an unknown.
    Leaves,
    /// Not derived, with the reason.
    NotDerived(OriginGap)
}

impl Provenance {
    /// The channel a positive answer names, as a stable key, or the gap's
    /// own key where the origin was not derived.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Written(_) => "written",
            Self::Parameter(_) => "parameter",
            Self::Read(_) => "read",
            Self::Computed(_) => "computed",
            Self::Chosen(_) => "chosen",
            Self::Called(_) => "called",
            Self::Field(_) => "field",
            Self::Constant(_) => "constant",
            Self::Leaves => "leaves",
            Self::NotDerived(gap) => gap.label()
        }
    }

    /// Whether the origin was settled at all.
    #[must_use]
    pub const fn derived(&self) -> bool {
        !matches!(self, Self::NotDerived(_))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::Provenance;
    use crate::{
        semantic::behavior::{OriginGap, behavior},
        source::Source
    };

    fn operands(text: &str, function: &str) -> Vec<Provenance> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        behavior(&sources)
            .iter()
            .find(|fact| fact.function == function)
            .and_then(|fact| fact.operations.first().map(|site| site.operands.clone()))
            .expect("the function records an operation")
    }

    /// A written type does not change which value a name stands for.
    /// Reading only the bare binding left `let args: Vec<String> = ..`
    /// foreign, and with it every clause resting on that value.
    #[test]
    fn a_name_bound_in_one_arm_never_answers_a_reading_in_another() {
        let text = "fn pick(choice: u8, given: u8) -> u8 { match choice { 1 => { let value = \
                    given; value } _ => { let value = 7; value } } }\n";
        let sources = [crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )];
        let facts = behavior(&sources);
        let alternatives: Vec<Provenance> = facts
            .iter()
            .flat_map(|fact| fact.choices.iter())
            .flat_map(|choice| choice.alternatives.iter().map(|arm| arm.value.clone()))
            .collect();
        assert!(alternatives.iter().any(|origin| matches!(origin, Provenance::Parameter(slot) if slot.name.as_deref() == Some("given"))));
        assert!(
            alternatives
                .iter()
                .any(|origin| matches!(origin, Provenance::Written(_)))
        );
    }

    #[test]
    fn a_loop_binding_stands_for_an_element_of_what_it_iterates() {
        let text = "fn total(values: Vec<u32>) -> u32 { let mut sum = 0; for value in values { \
                    sum += value; } sum }\n";
        let sources = [crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )];
        let facts = behavior(&sources);
        let selections = facts
            .iter()
            .flat_map(|fact| fact.operations.iter())
            .filter(|operation| {
                matches!(
                    operation.operation,
                    crate::semantic::behavior::Operation::Selection
                )
            })
            .count();
        assert_eq!(selections, 1);
        let operands: Vec<Provenance> = facts
            .iter()
            .flat_map(|fact| fact.operations.iter())
            .flat_map(|operation| operation.operands.clone())
            .collect();
        assert!(operands.iter().any(|origin| matches!(origin, Provenance::Parameter(slot) if slot.name.as_deref() == Some("values"))));
        assert!(operands.iter().any(Provenance::derived));
    }

    #[test]
    fn a_library_path_standing_for_a_value_is_settled_not_foreign() {
        let text = "fn limit() -> u32 { u32::MAX }\nfn absent() -> Option<u8> { None }\n";
        let sources = [crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )];
        let facts = behavior(&sources);
        let handbacks: Vec<Provenance> = facts
            .iter()
            .flat_map(|fact| fact.handbacks.iter().map(|handback| handback.value.clone()))
            .collect();
        assert!(handbacks.contains(&Provenance::Constant("u32::MAX".to_owned())));
        assert!(handbacks.contains(&Provenance::Constant("None".to_owned())));
        assert!(handbacks.iter().all(Provenance::derived));
    }

    #[test]
    fn a_binding_with_a_written_type_keeps_its_origin() {
        let bare = operands(
            "fn go(seed: u32) -> u32 { let start = seed; start + 1 }",
            "go"
        );
        let annotated = operands(
            "fn go(seed: u32) -> u32 { let start: u32 = seed; start + 1 }",
            "go"
        );
        assert_eq!(bare, annotated);
        assert!(annotated[0].derived());
    }

    /// A value read out of a field is named by that field, because that
    /// is what the body proves. Refusing here would call a recorded fact
    /// an absence.
    #[test]
    fn a_value_read_out_of_a_field_is_named_by_the_field() {
        let recorded = operands(
            "struct Pair { left: u32, right: u32 }\nimpl Pair { fn total(&self) -> u32 { \
             self.left + self.right } }",
            "Pair::total"
        );
        assert_eq!(
            recorded,
            vec![
                Provenance::Field("Pair.left".to_owned()),
                Provenance::Field("Pair.right".to_owned())
            ]
        );
    }

    /// A field whose owning type the tables cannot name is not an answer:
    /// naming the field alone would state of one type what was read off
    /// another.
    #[test]
    fn a_field_of_an_unnamed_type_refuses() {
        let recorded = operands(
            "fn total(holder: impl Sized) -> u32 { holder.left + 1 }",
            "total"
        );
        assert_eq!(recorded[0], Provenance::NotDerived(OriginGap::Foreign));
    }
}
