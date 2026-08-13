// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Feature binarization over the behavioral fact base.
//!
//! Turns one function's [`FunctionFacts`] into a fixed-order vector of
//! named binary features — the substrate the interpretable learning layer
//! trains on and the input format role-classification heuristics read.
//! Boolean predicates map directly (`returns-result`, `effect-net`);
//! counts are thermometer-encoded (`clones>=1`, `clones>=2`, `clones>=4`),
//! so a learned clause can express "at least N" without seeing raw
//! numbers.
//!
//! The vector is deterministic: same facts, same features, same order.
//! Every function yields the same feature names, so vectors from
//! different functions and different runs are directly comparable.

use super::behavior::{Effect, FunctionFacts, ParamMode};

/// One named binary feature of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// Stable feature name, e.g. `effect-net` or `clones>=2`.
    pub name:   &'static str,
    /// Whether the feature holds for the function.
    pub active: bool
}

/// Thermometer thresholds applied to every encoded count.
const THERMOMETER: [u32; 3] = [1, 2, 4];

/// Binarizes one function's facts into the fixed-order feature vector.
#[must_use]
pub fn features(facts: &FunctionFacts) -> Vec<Feature> {
    let owner = facts.function.split_once("::").map(|(owner, _)| owner);
    let mut out = Vec::with_capacity(43);
    out.push(feature("is-method", owner.is_some()));
    out.push(feature("is-trait-impl", facts.flags.trait_impl()));
    out.push(feature("returns-result", !facts.error_types.is_empty()));
    out.push(feature("constructs-types", !facts.constructs.is_empty()));
    for (name, effect) in [
        ("effect-unsafe", Effect::Unsafe),
        ("effect-fs", Effect::FileSystem),
        ("effect-net", Effect::Network),
        ("effect-process", Effect::Process),
        ("effect-env", Effect::Environment),
        ("effect-console", Effect::ConsoleIo),
        ("effect-db-read", Effect::DatabaseRead),
        ("effect-db-write", Effect::DatabaseWrite),
        ("effect-db", Effect::Database)
    ] {
        out.push(feature(name, facts.effects.contains(&effect)));
    }
    out.push(feature(
        "writes-own-field",
        writes_matching(facts, owner, true)
    ));
    out.push(feature(
        "writes-foreign-field",
        writes_matching(facts, owner, false)
    ));
    out.push(feature(
        "writes-unresolved-field",
        facts.writes.iter().any(|write| write.starts_with("?."))
    ));
    out.push(feature(
        "param-owned",
        param_with_mode(facts, ParamMode::Owned)
    ));
    out.push(feature(
        "param-borrowed",
        param_with_mode(facts, ParamMode::Borrowed)
    ));
    out.push(feature(
        "param-borrowed-mut",
        param_with_mode(facts, ParamMode::BorrowedMut)
    ));
    thermometer(
        &mut out,
        ["params>=1", "params>=2", "params>=4"],
        count(facts.params.len())
    );
    thermometer(
        &mut out,
        ["reads>=1", "reads>=2", "reads>=4"],
        count(facts.reads.len())
    );
    thermometer(
        &mut out,
        ["writes>=1", "writes>=2", "writes>=4"],
        count(facts.writes.len())
    );
    thermometer(
        &mut out,
        ["builds>=1", "builds>=2", "builds>=4"],
        count(facts.builds.len())
    );
    thermometer(
        &mut out,
        ["constructs>=1", "constructs>=2", "constructs>=4"],
        count(facts.constructs.len())
    );
    thermometer(
        &mut out,
        ["error-types>=1", "error-types>=2", "error-types>=4"],
        count(facts.error_types.len())
    );
    thermometer(
        &mut out,
        ["clones>=1", "clones>=2", "clones>=4"],
        facts.clones
    );
    thermometer(
        &mut out,
        ["allocations>=1", "allocations>=2", "allocations>=4"],
        facts.allocations
    );
    out
}

/// Builds one feature entry.
const fn feature(name: &'static str, active: bool) -> Feature {
    Feature {
        name,
        active
    }
}

/// Saturates a collection length into the count domain.
fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

/// Pushes one thermometer-encoded count as three threshold features.
fn thermometer(out: &mut Vec<Feature>, names: [&'static str; 3], value: u32) {
    for (name, threshold) in names.into_iter().zip(THERMOMETER) {
        out.push(feature(name, value >= threshold));
    }
}

/// Returns whether any write targets a field of the impl owner (`own`
/// true) or of a resolved type other than the owner (`own` false).
fn writes_matching(facts: &FunctionFacts, owner: Option<&str>, own: bool) -> bool {
    facts.writes.iter().any(|write| {
        let Some((target, _)) = write.split_once('.') else {
            return false;
        };
        if target == "?" {
            return false;
        }
        let matches_owner = owner == Some(target);
        if own { matches_owner } else { !matches_owner }
    })
}

/// Returns whether any parameter takes the given mode.
fn param_with_mode(facts: &FunctionFacts, mode: ParamMode) -> bool {
    facts.params.iter().any(|param| param.mode == mode)
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

    use super::features;
    use crate::{
        semantic::{behavior, settling::Settling},
        source::Source
    };

    fn first_features(text: &str) -> Vec<super::Feature> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        features(&behavior(&sources)[0])
    }

    /// The features of the first function of `text`, read with what
    /// `settle` states a resolver settled about it.
    fn first_features_settled<'text>(
        text: &'text str,
        settle: impl FnOnce(Settling<'text>) -> Settling<'text>
    ) -> Vec<super::Feature> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = settle(Settling::over("src/lib.rs", text)).done();
        features(&crate::semantic::behavior::behavior_with(&sources, Some(&resolution))[0])
    }

    fn active(all: &[super::Feature], name: &str) -> bool {
        all.iter().find(|entry| entry.name == name).unwrap().active
    }

    #[test]
    fn vector_has_a_fixed_deterministic_shape() {
        let empty = first_features("fn idle() {}");
        let busy = first_features(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn credit(&mut self, amount: i64) -> \
             Result<(), String> { self.balance += amount; println!(\"done\"); Ok(()) } }"
        );
        assert_eq!(empty.len(), 43);
        let names: Vec<&str> = empty.iter().map(|entry| entry.name).collect();
        let busy_names: Vec<&str> = busy.iter().map(|entry| entry.name).collect();
        assert_eq!(names, busy_names);
    }

    #[test]
    fn boolean_predicates_reflect_the_facts() {
        let all = first_features_settled(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn credit(&mut self, amount: i64) -> \
             Result<(), String> { self.balance += amount; println!(\"done\"); Ok(()) } }",
            |settling| settling.field("self.balance", "Wallet", "i64")
        );
        assert!(active(&all, "is-method"));
        assert!(active(&all, "returns-result"));
        assert!(active(&all, "effect-console"));
        assert!(active(&all, "writes-own-field"));
        assert!(!active(&all, "writes-foreign-field"));
        assert!(!active(&all, "effect-net"));
        assert!(!active(&all, "is-trait-impl"));
    }

    #[test]
    fn foreign_field_writes_are_distinguished_from_own() {
        let all = first_features_settled(
            "struct Wallet { balance: i64 }\nfn drain(wallet: &mut Wallet) { wallet.balance = 0; \
             }",
            |settling| settling.field("wallet.balance", "Wallet", "i64")
        );
        assert!(!active(&all, "is-method"));
        assert!(!active(&all, "writes-own-field"));
        assert!(active(&all, "writes-foreign-field"));
    }

    #[test]
    fn thermometer_encodes_at_least_thresholds() {
        let all = first_features(
            "fn chatty(text: &str) -> String {\n    let one = text.to_string();\n    let two = \
             text.to_string();\n    let three = text.to_string();\n    format!(\"{one}{two}{three}\")\n}"
        );
        assert!(active(&all, "clones>=1"));
        assert!(active(&all, "clones>=2"));
        assert!(!active(&all, "clones>=4"));
        assert!(active(&all, "allocations>=1"));
        assert!(!active(&all, "allocations>=2"));
    }

    #[test]
    fn param_modes_become_features() {
        let all = first_features("fn mix(owned: String, seen: &str, out: &mut Vec<u8>) {}");
        assert!(active(&all, "param-owned"));
        assert!(active(&all, "param-borrowed"));
        assert!(active(&all, "param-borrowed-mut"));
        assert!(active(&all, "params>=2"));
        assert!(!active(&all, "params>=4"));
    }
}
