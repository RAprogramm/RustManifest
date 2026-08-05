// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The vocabulary the capability tier speaks: what a module can do to the
//! world, and that answer paired with the site that proved it.
//!
//! Lives in its own module so the modules that classify and reach depend on
//! it directly instead of reaching back into the tier root, keeping this
//! part of the module graph acyclic — the same reason
//! [`Insight`](crate::semantic::insight) sits apart from the tier it serves.

use std::fmt;

use super::evidence::Evidence;

/// What a module can do to the world outside the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// Takes a local socket, so something outside can reach it.
    Serves,
    /// Dials a socket elsewhere, so it reaches something outside.
    Fetches,
    /// Reaches a database.
    Stores,
    /// Reads the process environment.
    Configures,
    /// Reads or writes the filesystem.
    Persists,
    /// Writes to a standard stream.
    Reports,
    /// Starts a child process.
    Spawns
}

impl fmt::Display for Capability {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = match self {
            Self::Serves => "serves",
            Self::Fetches => "fetches",
            Self::Stores => "stores",
            Self::Configures => "configures",
            Self::Persists => "persists",
            Self::Reports => "reports",
            Self::Spawns => "spawns"
        };
        out.write_str(verb)
    }
}

/// One capability together with the site that proved it.
///
/// The pair travels as one value because neither half is worth reporting
/// alone: a capability without its proof is an assertion, and a proof
/// without the capability it establishes says nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenCapability {
    /// What was proved.
    pub capability: Capability,
    /// The function and call that proved it.
    pub evidence:   Evidence
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_renders_as_the_verb_it_states() {
        assert_eq!(Capability::Serves.to_string(), "serves");
        assert_eq!(Capability::Fetches.to_string(), "fetches");
        assert_eq!(Capability::Stores.to_string(), "stores");
        assert_eq!(Capability::Configures.to_string(), "configures");
        assert_eq!(Capability::Persists.to_string(), "persists");
        assert_eq!(Capability::Reports.to_string(), "reports");
        assert_eq!(Capability::Spawns.to_string(), "spawns");
    }

    #[test]
    fn capabilities_order_as_declared() {
        let mut all = vec![Capability::Stores, Capability::Serves, Capability::Fetches];
        all.sort_unstable();
        assert_eq!(
            all,
            vec![Capability::Serves, Capability::Fetches, Capability::Stores]
        );
    }
}
