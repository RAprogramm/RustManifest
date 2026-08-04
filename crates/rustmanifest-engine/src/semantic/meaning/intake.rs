// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the program takes in, as statements about one entry point.
//!
//! Field for field the write claim, and graded the same way: how
//! certainly the call chain reaches the function holding the site, and
//! how certainly the site runs once it is reached, composed into one.
//! Every recorded read becomes a claim, the ones no chain reaches
//! included — dropping those would turn a fact about the call graph into
//! an empty statement about the program.

use crate::{
    semantic::behavior::{FunctionFacts, Grade, INPUT_SOURCE_SET, InputRead},
    source::Source
};

/// One statement about a value the program takes in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeClaim {
    /// Function holding the read site.
    pub function:   String,
    /// File the site lives in.
    pub file:       String,
    /// The recorded read.
    pub read:       InputRead,
    /// How certainly the owning function is reached from the entry.
    pub reach:      Grade,
    /// How certainly the site runs, both parts composed.
    pub execution:  Grade,
    /// Version of the recognizer list that produced this claim.
    pub source_set: u32
}

impl IntakeClaim {
    /// What this claim states, in one sentence.
    #[must_use]
    pub fn sentence(&self) -> String {
        format!(
            "{} {} of its {} in `{}`",
            self.opening(),
            self.read.selector,
            self.read.channel,
            self.function
        )
    }

    /// The opening words, fixed by how certainly the site runs.
    const fn opening(&self) -> &'static str {
        match self.execution {
            Grade::Certain => "the process unconditionally takes",
            Grade::Possible => "the process may take",
            Grade::Dead => "a site that can never run would take",
            Grade::Unreached => "a site no call chain from this entry reaches would take"
        }
    }
}

/// Builds the intake claims of one entry point, one per recorded read.
pub(super) fn intake_claims(
    facts: &[FunctionFacts],
    sources: &[Source],
    reach: &[Grade]
) -> Vec<IntakeClaim> {
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = sources.get(fact.source).map_or_else(String::new, |source| {
            source.path().to_string_lossy().replace('\\', "/")
        });
        for read in &fact.input_reads {
            claims.push(IntakeClaim {
                function:   fact.function.clone(),
                file:       file.clone(),
                read:       read.clone(),
                execution:  reach[index].min(read.guard.local_grade()),
                reach:      reach[index],
                source_set: INPUT_SOURCE_SET
            });
        }
    }
    claims
}
