// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one choice among written alternatives.
//!
//! The record is reserved before the alternatives are walked, so the
//! channel keeps body order and the condition every alternative carries
//! can already name the choice it belongs to. What each alternative
//! produces is filled in afterwards, once its own body has records to be
//! named by.
//!
//! Whether an alternative accepts every remaining value is read by the
//! same pattern reading the guard frame uses, so the two readings of one
//! alternative can never disagree.

use super::{
    FactCollector,
    pattern::{arm_condition, irrefutable}
};
use crate::semantic::behavior::{
    choice::{Alternative, Choice, Closure},
    output::Multiplicity,
    value::Provenance
};

impl FactCollector {
    /// Reserves the record of one choice, answering with its position in
    /// the channel.
    pub(super) fn reserve_choice(
        &mut self,
        span: &std::ops::Range<usize>,
        scrutinee: Provenance
    ) -> usize {
        let index = self.facts.choices.len();
        self.facts.choices.push(Choice {
            scrutinee,
            alternatives: Vec::new(),
            closure: Closure::Unproven,
            site: rustmanifest_schema::Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            },
            guard: self.site_guard(),
            conditions: self.site_conditions(),
            multiplicity: Multiplicity::Once
        });
        index
    }

    /// Records the choice a fallback call makes: whichever value the
    /// receiver holds, or the one written beside it where it holds none.
    ///
    /// A recorded choice and not a silent pick of one side: the source
    /// really does choose here, and stating only one of the two would put
    /// a certainty in the sentence the program does not have.
    pub(super) fn either(
        &mut self,
        span: &std::ops::Range<usize>,
        held: Provenance,
        fallback: Provenance
    ) -> Provenance {
        let index = self.reserve_choice(span, held.clone());
        if let Some(record) = self.facts.choices.get_mut(index) {
            record.closure = Closure::CatchAll;
            record.alternatives.push(Alternative {
                pattern:     None,
                irrefutable: false,
                conditional: false,
                value:       held
            });
            record.alternatives.push(Alternative {
                pattern:     None,
                irrefutable: true,
                conditional: false,
                value:       fallback
            });
        }
        Provenance::Chosen(u32::try_from(index).unwrap_or(u32::MAX))
    }

    /// Records one alternative of a reserved choice.
    pub(super) fn record_alternative(&mut self, choice: usize, arm: &syn::Arm) {
        use quote::ToTokens;

        let accepts_everything = irrefutable(&arm.pat);
        let conditional = arm_condition(&arm.pat).is_some();
        let value = self.origin_of(&arm.body);
        let alternative = Alternative {
            pattern: Some(arm.pat.to_token_stream().to_string()),
            irrefutable: accepts_everything,
            conditional,
            value
        };
        if let Some(record) = self.facts.choices.get_mut(choice) {
            if alternative.irrefutable && !alternative.conditional {
                record.closure = Closure::CatchAll;
            }
            record.alternatives.push(alternative);
        }
    }

    /// Records one branch of a conditional as an alternative of its
    /// choice.
    ///
    /// A branch has no pattern: what selects it is the condition already
    /// recorded as the choice's scrutinee. The else branch accepts every
    /// remaining value and carries no test of its own, which is what
    /// closes the set — the same proof a catch-all arm gives.
    pub(super) fn record_branch(&mut self, choice: usize, value: Provenance, otherwise: bool) {
        let alternative = Alternative {
            pattern: None,
            irrefutable: otherwise,
            conditional: !otherwise,
            value
        };
        if let Some(record) = self.facts.choices.get_mut(choice) {
            if otherwise {
                record.closure = Closure::CatchAll;
            }
            record.alternatives.push(alternative);
        }
    }

    /// Names the value a reserved choice produces, once its alternatives
    /// are recorded, and settles how its set is closed.
    ///
    /// A catch-all already answered while the alternatives were recorded
    /// and outranks this: it proves the set closed whatever the type is.
    /// Only where none was written does the written variant set get a
    /// chance to prove it.
    pub(super) fn settle_choice(
        &mut self,
        choice: usize,
        span: &std::ops::Range<usize>,
        decided: Option<&syn::Expr>
    ) {
        let over = decided.and_then(|expression| self.settled_expr_type(expression));
        if let Some(record) = self.facts.choices.get_mut(choice)
            && record.closure == Closure::Unproven
        {
            let patterns: Vec<&str> = record
                .alternatives
                .iter()
                .filter(|alternative| !alternative.conditional)
                .filter_map(|alternative| alternative.pattern.as_deref())
                .collect();
            if patterns.len() == record.alternatives.len()
                && super::super::choice::cover_a_standard_type(over.as_deref(), &patterns)
            {
                record.closure = Closure::StandardVariants;
            }
        }
        self.produced
            .entry((
                u32::try_from(span.start).unwrap_or(u32::MAX),
                u32::try_from(span.end).unwrap_or(u32::MAX)
            ))
            .or_insert_with(|| Provenance::Chosen(u32::try_from(choice).unwrap_or(u32::MAX)));
    }
}
