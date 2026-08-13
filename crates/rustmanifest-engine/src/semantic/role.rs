// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Deterministic role heuristics over the behavioral fact base.
//!
//! Assigns each function a coarse behavioral role derived purely from its
//! observed facts, in a fixed priority order: effects trump writes, writes
//! trump construction, construction trumps reads. The taxonomy is
//! deliberately conservative — it claims only what the facts prove — and
//! exists as the seed labeling for the interpretable learning layer,
//! which refines these coarse roles into project-specific ones
//! (handler, repository, service) once trained.

use std::fmt;

use super::behavior::{Effect, FunctionFacts};

/// Coarse behavioral role of one function, derived from its facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Touches the world outside the process: filesystem, network,
    /// processes, environment, or console.
    Io,
    /// Writes at least one field.
    Mutator,
    /// Constructs values without writing existing state or touching the
    /// outside world.
    Constructor,
    /// Only reads state: no writes, no effects, no construction.
    Accessor,
    /// No observed reads, writes, effects, or construction.
    Pure,
    /// The facts prove none of the above distinctions.
    Unknown
}

impl fmt::Display for Role {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Io => "io",
            Self::Mutator => "mutator",
            Self::Constructor => "constructor",
            Self::Accessor => "accessor",
            Self::Pure => "pure",
            Self::Unknown => "unknown"
        })
    }
}

/// Effects that reach outside the process.
const OUTSIDE_EFFECTS: [Effect; 7] = [
    Effect::FileSystem,
    Effect::Network,
    Effect::Process,
    Effect::Environment,
    Effect::ConsoleIo,
    Effect::DatabaseRead,
    Effect::DatabaseWrite
];

/// Derives the coarse role of one function from its facts.
#[must_use]
pub fn role(facts: &FunctionFacts) -> Role {
    if facts
        .effects
        .iter()
        .any(|effect| OUTSIDE_EFFECTS.contains(effect))
    {
        return Role::Io;
    }
    if !facts.writes.is_empty() {
        return Role::Mutator;
    }
    if !facts.constructs.is_empty() || !facts.builds.is_empty() {
        return Role::Constructor;
    }
    if !facts.reads.is_empty() {
        return Role::Accessor;
    }
    if facts.effects.is_empty() && facts.error_types.is_empty() {
        return Role::Pure;
    }
    Role::Unknown
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

    use super::{Role, role};
    use crate::{
        semantic::settling::Settling,
        source::Source
    };

    fn first_role(text: &str) -> Role {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        role(&crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::settled_over(&sources)))[0])
    }

    /// The role of the first function of `text`, read with what `settle`
    /// states a resolver settled about the fixture.
    fn first_role_settled<'text>(
        text: &'text str,
        settle: impl FnOnce(Settling<'text>) -> Settling<'text>
    ) -> Role {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = settle(Settling::over("src/lib.rs", text)).done();
        role(&crate::semantic::behavior_with(&sources, Some(&resolution))[0])
    }

    #[test]
    fn console_output_is_io() {
        assert_eq!(
            first_role("fn shout(text: &str) { println!(\"{text}\"); }"),
            Role::Io
        );
    }

    #[test]
    fn field_writers_are_mutators() {
        assert_eq!(
            first_role(
                "struct Wallet { balance: i64 }\nimpl Wallet { fn credit(&mut self, amount: \
                 i64) { self.balance += amount; } }"
            ),
            Role::Mutator
        );
    }

    #[test]
    fn struct_literal_builders_are_constructors() {
        assert_eq!(
            first_role(
                "struct Wallet { balance: i64 }\nimpl Wallet { fn new() -> Self { Self { \
                 balance: 0 } } }"
            ),
            Role::Constructor
        );
    }

    /// A body writing into a value it made itself assembles that value,
    /// and what the value is comes from the type the compiler settled for
    /// the call that made it.
    #[test]
    fn fresh_value_assemblers_are_constructors() {
        assert_eq!(
            first_role_settled(
                "struct Order { items: u32 }\nfn fetch() -> Order { let mut order = \
                 Order::empty(); order.items = 3; order }\nimpl Order { fn empty() -> Self { \
                 Self { items: 0 } } }",
                |settling| {
                    settling
                        .field("order.items", "Order", "u32")
                        .call("Order::empty()", "empty", "Order", None)
                }
            ),
            Role::Constructor
        );
    }

    #[test]
    fn readers_are_accessors() {
        assert_eq!(
            first_role(
                "struct Wallet { balance: i64 }\nimpl Wallet { fn balance(&self) -> i64 { \
                 self.balance } }"
            ),
            Role::Accessor
        );
    }

    #[test]
    fn effectless_computation_is_pure() {
        assert_eq!(
            first_role("fn double(value: u64) -> u64 { value * 2 }"),
            Role::Pure
        );
    }

    #[test]
    fn io_trumps_writes() {
        assert_eq!(
            first_role(
                "struct Wallet { balance: i64 }\nimpl Wallet { fn drain(&mut self) { \
                 self.balance = 0; println!(\"drained\"); } }"
            ),
            Role::Io
        );
    }

    #[test]
    fn unsafe_only_functions_are_unknown() {
        assert_eq!(first_role("unsafe fn poke() {}"), Role::Unknown);
    }
}
