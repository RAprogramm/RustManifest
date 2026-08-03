// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What stands between one site and unconditional execution: the
//! constructs enclosing it, the statements that may leave before it, and
//! the proof that it never runs at all.

use super::grade::Grade;

/// One construct standing between a write and unconditional execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// The then-branch of a conditional.
    If,
    /// The else-branch of a conditional.
    Else,
    /// One arm of a match.
    MatchArm {
        /// Whether the arm's pattern accepts every remaining value.
        irrefutable: bool,
        /// Whether the arm carries an additional condition.
        guarded:     bool
    },
    /// A loop whose body may run zero times.
    For,
    /// A conditional loop whose body may run zero times.
    While,
    /// A body that runs only where something drives it: a closure where
    /// it is called, an `async` block where its future is polled. The
    /// two are one frame because they weaken a write for one reason —
    /// the body is written here and run elsewhere, if at all.
    Closure,
    /// An item nested inside a body, which runs only when called.
    NestedItem,
    /// A configuration gate this tier does not resolve.
    CfgGated
}

/// Everything standing between one write site and certainty.
///
/// The two questions are distinct and are answered separately: `dead` is
/// a proof that the site never runs, `prior_exits` counts what stands
/// between the start of the function and the site without proving
/// either way. Neither being set is the third state — the site is
/// proven to be reached.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteGuard {
    /// Enclosing constructs, outermost first.
    pub guards:      Vec<Guard>,
    /// Statements before this site that may not hand control to it: an
    /// exit that may leave the function first, a divergence proven on
    /// some paths but not all, or a construct whose completion this tier
    /// does not resolve. Provenance for the reading, never the reading
    /// itself — one such statement weakens the site exactly as much as
    /// ten do.
    pub prior_exits: u32,
    /// Whether the site follows a proven divergence and can never run.
    pub dead:        bool
}

impl SiteGuard {
    /// A site under nothing at all.
    #[must_use]
    pub fn unconditional() -> Self {
        Self::default()
    }

    /// How certainly this site runs when its own function runs.
    #[must_use]
    pub const fn local_grade(&self) -> Grade {
        if self.dead {
            return Grade::Dead;
        }
        if self.guards.is_empty() && self.prior_exits == 0 {
            return Grade::Certain;
        }
        Grade::Possible
    }
}

#[cfg(test)]
mod tests {
    use super::{Grade, Guard, SiteGuard};

    #[test]
    fn a_site_is_certain_only_without_guards_and_without_prior_exits() {
        assert_eq!(SiteGuard::unconditional().local_grade(), Grade::Certain);
        let guarded = SiteGuard {
            guards:      vec![Guard::If],
            prior_exits: 0,
            dead:        false
        };
        assert_eq!(guarded.local_grade(), Grade::Possible);
        let after_exit = SiteGuard {
            guards:      Vec::new(),
            prior_exits: 1,
            dead:        false
        };
        assert_eq!(after_exit.local_grade(), Grade::Possible);
        let unreachable = SiteGuard {
            guards:      Vec::new(),
            prior_exits: 0,
            dead:        true
        };
        assert_eq!(unreachable.local_grade(), Grade::Dead);
    }
}
