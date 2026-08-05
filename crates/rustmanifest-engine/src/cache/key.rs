// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The content address of a cache entry: a 128-bit FNV-1a hash of the file
//! bytes followed by the rule-set fingerprint.
//!
//! The width matters. A 64-bit address collides by the birthday bound after
//! roughly four billion distinct inputs, and a collision here is not a miss
//! — it silently serves one file's findings for another. At 128 bits the
//! same bound sits at 2^64 distinct inputs, past any corpus this engine will
//! ever see, and the entry additionally records the lengths it was built
//! from so a mismatch is caught and treated as a miss.

use rustmanifest_schema::Rule;

/// FNV-1a 128-bit offset basis.
const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;

/// FNV-1a 128-bit prime, `2^88 + 2^8 + 0x3b`.
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

/// Hashes `bytes` with the FNV-1a 128-bit algorithm.
fn fnv1a128(bytes: impl IntoIterator<Item = u8>) -> u128 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Builds the rule-set half of the cache key.
///
/// The fingerprint is the sorted rule-id set, followed by every rule (sorted
/// by id) serialized as JSON, followed by the engine crate version. Each
/// component is NUL-separated so that adjacent components cannot collide by
/// concatenation.
///
/// A rule that fails to serialize contributes an empty component; the
/// fingerprint stays deterministic either way.
#[must_use]
pub fn rules_fingerprint(rules: &[&Rule]) -> Vec<u8> {
    let mut sorted: Vec<&Rule> = rules.to_vec();
    sorted.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    let mut bytes = Vec::new();
    for rule in &sorted {
        bytes.extend_from_slice(rule.id.as_bytes());
        bytes.push(0);
    }
    for rule in &sorted {
        bytes.extend_from_slice(&serde_json::to_vec(rule).unwrap_or_default());
        bytes.push(0);
    }
    bytes.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
    bytes
}

/// Computes the entry key for one file: the hex-encoded FNV-1a 128-bit hash
/// of `file_bytes` followed by `rules_fingerprint`.
#[must_use]
pub(super) fn of(file_bytes: &[u8], rules_fingerprint: &[u8]) -> String {
    let hash = fnv1a128(file_bytes.iter().chain(rules_fingerprint).copied());
    format!("{hash:032x}")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use rustmanifest_schema::{RuleDefinition, Severity};

    use super::*;

    /// Vectors from the FNV reference implementation for the 128-bit
    /// variant.
    #[test]
    fn fnv1a128_matches_reference_vectors() {
        assert_eq!(fnv1a128([]), FNV_OFFSET_BASIS);
        assert_eq!(fnv1a128(*b"a"), 0xd228_cb69_6f1a_8caf_7891_2b70_4e4a_8964);
        assert_eq!(
            fnv1a128(*b"foobar"),
            0x343e_1662_793c_64bf_6f0d_3597_ba44_6f18
        );
    }

    #[test]
    fn key_is_thirty_two_hex_chars_and_input_sensitive() {
        let base = of(b"contents", b"rules");
        assert_eq!(base.len(), 32);
        assert!(base.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(base, of(b"contents!", b"rules"));
        assert_ne!(base, of(b"contents", b"rules!"));
    }

    fn sample_rule(id: &str, regex: &str) -> Rule {
        Rule {
            id:            id.to_owned(),
            severity:      Severity::Error,
            title:         "sample".to_owned(),
            rationale_uri: "rustmanifest://methodology/sample".to_owned(),
            definition:    RuleDefinition::Pattern {
                regex:         regex.to_owned(),
                exclude_globs: vec![]
            }
        }
    }

    #[test]
    fn rules_fingerprint_is_order_independent_but_content_sensitive() {
        let first = sample_rule("RM-AAA-001", "foo");
        let second = sample_rule("RM-BBB-002", "bar");
        let forward = rules_fingerprint(&[&first, &second]);
        assert_eq!(forward, rules_fingerprint(&[&second, &first]));

        let changed = sample_rule("RM-BBB-002", "baz");
        assert_ne!(forward, rules_fingerprint(&[&first, &changed]));
        assert_ne!(forward, rules_fingerprint(&[&first]));
    }
}
