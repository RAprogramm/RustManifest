// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The position a fact is recorded at: the stack of constructs standing
//! over it, the repetition stamped on the writes of a repeating body, and
//! the snapshot both are read off.

use super::FactCollector;
use crate::semantic::behavior::output::{Guard, Multiplicity, SiteGuard};

impl FactCollector {
    /// Visits `body` with one more enclosing construct on the stack, so
    /// every write recorded inside it carries that construct.
    pub(super) fn under<F: FnOnce(&mut Self)>(&mut self, guard: Guard, body: F) {
        self.guards.push(guard);
        body(self);
        self.guards.pop();
    }

    /// Visits `body` as the body of a repeating construct, stamping
    /// every write recorded inside it with `multiplicity`.
    ///
    /// Repetition is recorded here rather than read off the guard stack,
    /// because repeating and weakening are different questions: a `loop`
    /// repeats without bound while its body is still entered
    /// unconditionally. It is also not stamped for standing inside a
    /// loop: `multiplicity` comes from the completion analysis of the
    /// body, so a body that provably leaves on its first pass keeps the
    /// single run it has.
    pub(super) fn repeating<F: FnOnce(&mut Self)>(&mut self, multiplicity: Multiplicity, body: F) {
        let recorded = self.facts.output_writes.len();
        body(self);
        if matches!(multiplicity, Multiplicity::Once) {
            return;
        }
        for write in self.facts.output_writes.iter_mut().skip(recorded) {
            write.multiplicity = multiplicity;
        }
    }

    /// Snapshot of everything standing between the current position and
    /// unconditional execution.
    pub(super) fn site_guard(&self) -> SiteGuard {
        SiteGuard {
            guards:      self.guards.clone(),
            prior_exits: self.prior_exits,
            dead:        self.dead
        }
    }
}
