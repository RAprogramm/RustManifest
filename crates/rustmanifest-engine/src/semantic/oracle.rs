// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The compiler-knowledge overlay for traits: what the compiler's own
//! documentation index says a type implements.
//!
//! The syntactic tier answers "which traits" only from what is written
//! in the workspace; auto traits and blanket implementations are the
//! compiler's to hand out, and no text reading can claim them. This
//! overlay reads the JSON index rustdoc emits (`cargo rustdoc --
//! --output-format json`) and records, per type, every implementation the
//! compiler lists — written, auto, or blanket — with its provenance. Like
//! the SCIP overlay, it is an opt-in oracle: absent index, absent answers,
//! and every claim it feeds a surface names the index as its source.
//!
//! The index is read through the shapes the emitter itself publishes, so
//! an emission of a format this build does not know is refused rather
//! than half-read. Reading it as untyped values accepted any version and
//! answered from whichever fields happened to still be spelled the same,
//! which is a silent wrong answer where a refusal belongs.

use std::collections::BTreeMap;

use rustdoc_types::{Crate, Impl, ItemEnum, Type};

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
    /// Reads one rustdoc JSON emission, or nothing where the text is not
    /// one this build knows the shape of.
    ///
    /// The format the emitter writes is versioned, and a version this
    /// build was not compiled against is refused: the emitter is free to
    /// rename a field between versions, and a reading that guesses past
    /// that produces claims about implementations nobody wrote.
    #[must_use]
    pub fn from_rustdoc_json(text: &str) -> Option<Self> {
        let emitted: Crate = serde_json::from_str(text).ok()?;
        if emitted.format_version != rustdoc_types::FORMAT_VERSION {
            return None;
        }
        let mut listed: BTreeMap<String, Vec<OracleImpl>> = BTreeMap::new();
        for item in emitted.index.values() {
            let ItemEnum::Impl(written) = &item.inner else {
                continue;
            };
            let (Some(trait_name), Some(type_name)) =
                (implemented_trait(written), implemented_for(&written.for_))
            else {
                continue;
            };
            listed.entry(type_name).or_default().push(OracleImpl {
                trait_name,
                provenance: provenance_of(written)
            });
        }
        (!listed.is_empty()).then_some(Self {
            listed
        })
    }

    /// Merges another emission's listings into this one, so a
    /// multi-crate workspace answers from every documented crate rather
    /// than the crate documented last; a listing both emissions carry
    /// lands once.
    #[must_use]
    pub fn merged(mut self, other: Self) -> Self {
        for (type_name, impls) in other.listed {
            let held = self.listed.entry(type_name).or_default();
            for one in impls {
                if !held.contains(&one) {
                    held.push(one);
                }
            }
        }
        self
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

/// The last segment of the trait an impl implements, or `None` for an
/// inherent one.
fn implemented_trait(written: &Impl) -> Option<String> {
    last_segment(&written.trait_.as_ref()?.path)
}

/// The last segment of the type an impl is written for, where that type
/// is a named one.
fn implemented_for(target: &Type) -> Option<String> {
    match target {
        Type::ResolvedPath(path) => last_segment(&path.path),
        _ => None
    }
}

/// Where the compiler says an implementation comes from.
const fn provenance_of(written: &Impl) -> OracleProvenance {
    if written.is_synthetic {
        OracleProvenance::Auto
    } else if written.blanket_impl.is_some() {
        OracleProvenance::Blanket
    } else {
        OracleProvenance::Written
    }
}

/// The name a path ends in, empty paths excluded.
fn last_segment(path: &str) -> Option<String> {
    let named = path.rsplit("::").next()?;
    (!named.is_empty()).then(|| named.to_owned())
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

    /// A real emission of this build's format, captured from `cargo
    /// rustdoc -- --output-format json` over a probe crate declaring one
    /// type with one written implementation. The compiler adds the auto
    /// trait and the blanket implementation itself, so all three
    /// provenances are real rather than composed here.
    const EMITTED: &str = include_str!("oracle/emission.json");

    /// The same emission with its version number moved off the one this
    /// build knows.
    fn of_another_format() -> String {
        EMITTED.replace(
            &format!("\"format_version\": {}", rustdoc_types::FORMAT_VERSION),
            &format!("\"format_version\": {}", rustdoc_types::FORMAT_VERSION - 1)
        )
    }

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
            oracle.implements("User", "ToString").unwrap().provenance,
            OracleProvenance::Blanket
        );
    }

    /// An emission of a format this build does not know is refused. Read
    /// as untyped values it would answer from whichever fields still
    /// happened to match, which is a claim about implementations from a
    /// shape nobody verified.
    #[test]
    fn an_emission_of_an_unknown_format_is_refused_rather_than_half_read() {
        assert!(TraitOracle::from_rustdoc_json(&of_another_format()).is_none());
    }

    #[test]
    fn text_that_is_no_emission_answers_nothing() {
        assert!(TraitOracle::from_rustdoc_json("{}").is_none());
        assert!(TraitOracle::from_rustdoc_json("not json").is_none());
    }

    #[test]
    fn a_type_the_index_does_not_know_is_told_apart_from_one_with_no_impls() {
        let oracle = TraitOracle::from_rustdoc_json(EMITTED).unwrap();
        assert!(!oracle.knows("Absent"));
        assert_eq!(oracle.traits_of("Absent"), []);
    }
}
