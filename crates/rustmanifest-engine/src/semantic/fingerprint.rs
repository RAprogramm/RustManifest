// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Name-independent behavior fingerprints.
//!
//! The fingerprint hashes what a function *does* — parameter modes and
//! types, reads, writes, constructions, error types, effects, calls, and
//! counts — but never its own name or location. Two snapshots of a
//! workspace can therefore prove a rename: the old name disappears, a new
//! name appears with the same fingerprint, and the diff is a repair of
//! naming, not a deletion plus an addition.

use super::behavior::FunctionFacts;

/// 64-bit FNV-1a offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// 64-bit FNV-1a prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Computes the name-independent fingerprint of one function's behavior.
#[must_use]
pub fn behavior_fingerprint(fact: &FunctionFacts) -> u64 {
    let mut hash = FNV_OFFSET;
    let mut eat = |text: &str| {
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash ^= u64::from(b'\x1f');
        hash = hash.wrapping_mul(FNV_PRIME);
    };
    for param in &fact.params {
        eat(&param.mode.to_string());
        eat(&param.type_name);
    }
    for list in [
        &fact.reads,
        &fact.writes,
        &fact.builds,
        &fact.constructs,
        &fact.error_types,
        &fact.calls
    ] {
        for entry in list {
            eat(entry);
        }
        eat("|");
    }
    for effect in &fact.effects {
        eat(&effect.to_string());
    }
    eat(&fact.clones.to_string());
    eat(&fact.allocations.to_string());
    hash
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

    use super::behavior_fingerprint;
    use crate::{semantic::behavior, source::Source};

    fn prints(text: &str) -> Vec<u64> {
        behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
            .iter()
            .map(behavior_fingerprint)
            .collect()
    }

    #[test]
    fn renaming_a_function_keeps_its_fingerprint() {
        let before = prints("struct W { b: i64 }\nfn credit(w: &mut W, a: i64) { w.b += a; }");
        let after = prints("struct W { b: i64 }\nfn deposit(w: &mut W, a: i64) { w.b += a; }");
        assert_eq!(before, after);
    }

    #[test]
    fn changing_behavior_changes_the_fingerprint() {
        let before = prints("struct W { b: i64 }\nfn credit(w: &mut W, a: i64) { w.b += a; }");
        let after = prints(
            "struct W { b: i64 }\nfn credit(w: &mut W, a: i64) { w.b -= 0; \
                            println!(\"{a}\"); }"
        );
        assert_ne!(before, after);
    }

    #[test]
    fn fingerprints_are_deterministic() {
        let text = "fn compute(v: &[u64]) -> u64 { v.iter().sum() }";
        assert_eq!(prints(text), prints(text));
    }
}
