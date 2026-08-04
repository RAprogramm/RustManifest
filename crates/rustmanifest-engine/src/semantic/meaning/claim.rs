// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The three remaining statements about one entry point: what it
//! computes, what it chooses between, and what it hands back.
//!
//! Field for field the write claim, and graded the same way — how
//! certainly the call chain reaches the function holding the site,
//! composed with how certainly the site runs once it is reached. Every
//! recorded site becomes a claim, the ones no chain reaches included:
//! dropping those would turn a fact about the call graph into an empty
//! statement about the program.

use crate::{
    semantic::behavior::{
        CHOICE_SET, Choice, FunctionFacts, Grade, HANDBACK_SET, Handback, OPERATION_SET,
        OperationSite
    },
    source::Source
};

/// One statement about a value the program computes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeClaim {
    /// Function holding the site.
    pub function:      String,
    /// File the site lives in.
    pub file:          String,
    /// The recorded operation.
    pub operation:     OperationSite,
    /// How certainly the owning function is reached from the entry.
    pub reach:         Grade,
    /// How certainly the site runs, both parts composed.
    pub execution:     Grade,
    /// Version of the recognizer list that produced this claim.
    pub operation_set: u32
}

/// One statement about what the program chooses between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceClaim {
    /// Function holding the site.
    pub function:   String,
    /// File the site lives in.
    pub file:       String,
    /// The recorded choice.
    pub choice:     Choice,
    /// How certainly the owning function is reached from the entry.
    pub reach:      Grade,
    /// How certainly the site runs, both parts composed.
    pub execution:  Grade,
    /// Version of the recognizer list that produced this claim.
    pub choice_set: u32
}

/// One statement about a value the program hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandbackClaim {
    /// Function holding the position.
    pub function:     String,
    /// File the position lives in.
    pub file:         String,
    /// The recorded position.
    pub handback:     Handback,
    /// How certainly the owning function is reached from the entry.
    pub reach:        Grade,
    /// How certainly the position runs, both parts composed.
    pub execution:    Grade,
    /// Version of the recognizer list that produced this claim.
    pub handback_set: u32
}

/// The path of the source one fact was read from.
fn origin(sources: &[Source], fact: &FunctionFacts) -> String {
    sources.get(fact.source).map_or_else(String::new, |source| {
        source.path().to_string_lossy().replace('\\', "/")
    })
}

/// Builds the compute claims of one entry point.
pub(super) fn compute_claims(
    facts: &[FunctionFacts],
    sources: &[Source],
    reach: &[Grade]
) -> Vec<ComputeClaim> {
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = origin(sources, fact);
        for operation in &fact.operations {
            claims.push(ComputeClaim {
                function:      fact.function.clone(),
                file:          file.clone(),
                execution:     reach[index].min(operation.guard.local_grade()),
                reach:         reach[index],
                operation:     operation.clone(),
                operation_set: OPERATION_SET
            });
        }
    }
    claims
}

/// Builds the choice claims of one entry point.
pub(super) fn choice_claims(
    facts: &[FunctionFacts],
    sources: &[Source],
    reach: &[Grade]
) -> Vec<ChoiceClaim> {
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = origin(sources, fact);
        for choice in &fact.choices {
            claims.push(ChoiceClaim {
                function:   fact.function.clone(),
                file:       file.clone(),
                execution:  reach[index].min(choice.guard.local_grade()),
                reach:      reach[index],
                choice:     choice.clone(),
                choice_set: CHOICE_SET
            });
        }
    }
    claims
}

/// Builds the handback claims of one entry point.
pub(super) fn handback_claims(
    facts: &[FunctionFacts],
    sources: &[Source],
    reach: &[Grade]
) -> Vec<HandbackClaim> {
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = origin(sources, fact);
        for handback in &fact.handbacks {
            claims.push(HandbackClaim {
                function:     fact.function.clone(),
                file:         file.clone(),
                execution:    reach[index].min(handback.guard.local_grade()),
                reach:        reach[index],
                handback:     handback.clone(),
                handback_set: HANDBACK_SET
            });
        }
    }
    claims
}
