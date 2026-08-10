// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The compiler-knowledge overlay for traits: what the compiler's own
//! documentation index says a type implements.
//!
//! The syntactic tier answers "which traits" only from what is written
//! in the workspace; auto traits and blanket implementations are the
//! compiler's to hand out, and no text reading can claim them. This
//! overlay reads the JSON index rustdoc emits (`cargo rustdoc --
//! --output-format json`, format version pinned by test against a real
//! emission) and records, per type, every implementation the compiler
//! lists — written, auto, or blanket — with its provenance. Like the
//! SCIP overlay, it is an opt-in oracle: absent index, absent answers,
//! and every claim it feeds a surface names the index as its source.

use std::collections::BTreeMap;

/// Where one compiler-listed implementation comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleProvenance {
    /// An impl written in source (a derive expansion included).
    Written,
    /// An auto trait the compiler grants (`Send`, `Sync`, `Unpin`, …).
    Auto,
    /// A blanket impl reaching the type through a generic parameter.
    Blanket
}

/// One implementation the compiler's index lists for a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleImpl {
    /// Last path segment of the implemented trait.
    pub trait_name: String,
    /// Where the implementation comes from.
    pub provenance: OracleProvenance
}

/// Every implementation the compiler's index lists, per type name.
#[derive(Debug, Default)]
pub struct TraitOracle {
    listed: BTreeMap<String, Vec<OracleImpl>>
}

impl TraitOracle {
    /// Reads one rustdoc JSON emission, or nothing where the text is
    /// not one.
    #[must_use]
    pub fn from_rustdoc_json(text: &str) -> Option<Self> {
        let parsed: serde_json::Value = serde_json::from_str(text).ok()?;
        let index = parsed.get("index")?.as_object()?;
        let mut listed: BTreeMap<String, Vec<OracleImpl>> = BTreeMap::new();
        for item in index.values() {
            let Some(written) = item.get("inner").and_then(|inner| inner.get("impl")) else {
                continue;
            };
            let Some(trait_name) = written
                .get("trait")
                .and_then(|held| held.get("path"))
                .and_then(serde_json::Value::as_str)
                .and_then(|path| path.rsplit("::").next())
            else {
                continue;
            };
            let Some(type_name) = written
                .get("for")
                .and_then(|held| held.get("resolved_path"))
                .and_then(|held| held.get("path"))
                .and_then(serde_json::Value::as_str)
                .and_then(|path| path.rsplit("::").next())
            else {
                continue;
            };
            let provenance = if written
                .get("is_synthetic")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                OracleProvenance::Auto
            } else if written
                .get("blanket_impl")
                .is_some_and(|held| !held.is_null())
            {
                OracleProvenance::Blanket
            } else {
                OracleProvenance::Written
            };
            listed
                .entry(type_name.to_owned())
                .or_default()
                .push(OracleImpl {
                    trait_name: trait_name.to_owned(),
                    provenance
                });
        }
        if listed.is_empty() {
            return None;
        }
        Some(Self {
            listed
        })
    }

    /// The implementations listed for `type_name`, empty where the
    /// index does not know the type.
    #[must_use]
    pub fn traits_of(&self, type_name: &str) -> &[OracleImpl] {
        self.listed.get(type_name).map_or(&[], Vec::as_slice)
    }

    /// The listed implementation of `trait_name` for `type_name`.
    #[must_use]
    pub fn implements(&self, type_name: &str, trait_name: &str) -> Option<&OracleImpl> {
        self.traits_of(type_name)
            .iter()
            .find(|held| held.trait_name == trait_name)
    }

    /// Whether the index knows `type_name` at all — the difference
    /// between "no impl listed" and "no answer".
    #[must_use]
    pub fn knows(&self, type_name: &str) -> bool {
        self.listed.contains_key(type_name)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{OracleProvenance, TraitOracle};

    /// A trimmed real emission: rustdoc JSON format version 57, the
    /// shapes exactly as `cargo rustdoc -- --output-format json` wrote
    /// them for a probe crate.
    const EMITTED: &str = r#"{
        "format_version": 57,
        "index": {
            "10": {"inner": {"impl": {
                "trait": {"path": "Display", "id": 1, "args": null},
                "for": {"resolved_path": {"path": "User", "id": 3, "args": null}},
                "is_synthetic": false,
                "blanket_impl": null
            }}},
            "11": {"inner": {"impl": {
                "trait": {"path": "Sync", "id": 2, "args": null},
                "for": {"resolved_path": {"path": "User", "id": 3, "args": null}},
                "is_synthetic": true,
                "blanket_impl": null
            }}},
            "12": {"inner": {"impl": {
                "trait": {"path": "From", "id": 4, "args": null},
                "for": {"resolved_path": {"path": "User", "id": 3, "args": null}},
                "is_synthetic": false,
                "blanket_impl": {"generic": "T"}
            }}},
            "13": {"inner": {"struct": {"name": "User"}}}
        }
    }"#;

    #[test]
    fn a_real_emission_is_read_with_provenance() {
        let oracle = TraitOracle::from_rustdoc_json(EMITTED).unwrap();
        assert!(oracle.knows("User"));
        assert_eq!(
            oracle.implements("User", "Display").unwrap().provenance,
            OracleProvenance::Written
        );
        assert_eq!(
            oracle.implements("User", "Sync").unwrap().provenance,
            OracleProvenance::Auto
        );
        assert_eq!(
            oracle.implements("User", "From").unwrap().provenance,
            OracleProvenance::Blanket
        );
        assert!(oracle.implements("User", "Default").is_none());
        assert!(!oracle.knows("Ghost"));
    }

    #[test]
    fn a_text_that_is_no_index_answers_nothing() {
        assert!(TraitOracle::from_rustdoc_json("{}").is_none());
        assert!(TraitOracle::from_rustdoc_json("not json").is_none());
    }
}
