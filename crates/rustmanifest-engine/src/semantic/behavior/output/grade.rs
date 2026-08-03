// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How certainly something happens, as an ordered reading that composes:
//! two guarantees taken together are worth the weaker of the two.

use std::fmt;

/// How certainly something happens. The order is load-bearing: composing
/// two guarantees takes the weaker of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    /// Provably never runs.
    Dead,
    /// Not reachable from the entry point under analysis.
    Unreached,
    /// May run; something stands between it and certainty.
    Possible,
    /// Runs on every path.
    Certain
}

impl fmt::Display for Grade {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Dead => "dead",
            Self::Unreached => "unreached",
            Self::Possible => "possible",
            Self::Certain => "certain"
        };
        out.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::Grade;

    #[test]
    fn grades_order_from_dead_to_certain() {
        assert!(Grade::Dead < Grade::Unreached);
        assert!(Grade::Unreached < Grade::Possible);
        assert!(Grade::Possible < Grade::Certain);
        assert_eq!(Grade::Possible.min(Grade::Certain), Grade::Possible);
    }
}
