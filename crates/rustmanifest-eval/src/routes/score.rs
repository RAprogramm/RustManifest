// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Precision and recall of a recovered route table against the table a
//! project declares.
//!
//! Both numbers are kept, never merged into one score: they answer
//! different questions and a single figure hides which of the two failed.
//! Precision asks whether a recovered row is real; recall asks whether a
//! real row was recovered. A table that recovers one correct route out of
//! two hundred scores perfectly on the first and badly on the second, and a
//! reader must see that.
//!
//! The disagreeing rows are carried with the counts. A measurement that
//! reports only totals cannot be acted on; one that names the endpoints can.
//!
//! Both sides are named for what they are and not for a verdict. An
//! *undeclared* row is one the engine recovered and the project did not
//! annotate: that is a recovery the ground truth does not cover as often as
//! it is an invention, and calling it spurious would decide the question the
//! measurement exists to ask. An *unrecovered* row is one the project
//! declares and the engine did not produce, which likewise may be a gap in
//! the recovery or a path the two sides spell differently.

use std::collections::BTreeSet;

use serde::Serialize;

use super::declared::Endpoint;

/// How many endpoints each table holds and how many they share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Counts {
    /// Endpoints the project declares.
    pub declared:  usize,
    /// Endpoints the engine recovered.
    pub recovered: usize,
    /// Endpoints in both tables.
    pub agreed:    usize
}

/// Which endpoints the two tables disagree about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Disagreement {
    /// Declared endpoints the engine did not recover.
    pub unrecovered: Vec<String>,
    /// Recovered endpoints the project does not declare.
    pub undeclared:  Vec<String>
}

/// How a recovered route table compares against a declared one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Score {
    /// How much each table holds.
    pub counts:       Counts,
    /// Which endpoints they disagree about.
    pub disagreement: Disagreement
}

impl Score {
    /// Compares `recovered` against `declared`.
    #[must_use]
    pub fn of(declared: &BTreeSet<Endpoint>, recovered: &BTreeSet<Endpoint>) -> Self {
        Self {
            counts:       Counts {
                declared:  declared.len(),
                recovered: recovered.len(),
                agreed:    declared.intersection(recovered).count()
            },
            disagreement: Disagreement {
                unrecovered: rendered(declared.difference(recovered)),
                undeclared:  rendered(recovered.difference(declared))
            }
        }
    }

    /// Share of recovered endpoints that the project also declares, or
    /// `None` when nothing was recovered and the question does not arise.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        ratio(self.counts.agreed, self.counts.recovered)
    }

    /// Share of declared endpoints the engine also recovered, or `None`
    /// when nothing was declared and there is no ground truth to score
    /// against.
    #[must_use]
    pub fn recall(&self) -> Option<f64> {
        ratio(self.counts.agreed, self.counts.declared)
    }
}

/// Divides `part` by `whole`, answering `None` rather than a figure when
/// the denominator is zero.
fn ratio(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| {
        let part: u32 = u32::try_from(part).unwrap_or(u32::MAX);
        let whole: u32 = u32::try_from(whole).unwrap_or(u32::MAX);
        f64::from(part) / f64::from(whole)
    })
}

/// Renders endpoints as `METHOD path`, sorted by the set's own order.
fn rendered<'a>(endpoints: impl Iterator<Item = &'a Endpoint>) -> Vec<String> {
    endpoints
        .map(|(method, path)| format!("{method} {path}"))
        .collect()
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

    fn set(entries: &[(&str, &str)]) -> BTreeSet<Endpoint> {
        entries
            .iter()
            .map(|(method, path)| ((*method).to_owned(), (*path).to_owned()))
            .collect()
    }

    #[test]
    fn two_identical_tables_agree_completely() {
        let table = set(&[("GET", "/a"), ("POST", "/b")]);
        let score = Score::of(&table, &table);
        assert_eq!(score.counts.agreed, 2);
        assert_eq!(score.precision(), Some(1.0));
        assert_eq!(score.recall(), Some(1.0));
        assert_eq!(
            score.disagreement.unrecovered,
            [] as [std::string::String; 0]
        );
        assert_eq!(
            score.disagreement.undeclared,
            [] as [std::string::String; 0]
        );
    }

    #[test]
    fn a_unrecovered_endpoint_lowers_recall_and_is_named() {
        let score = Score::of(
            &set(&[("GET", "/a"), ("POST", "/b")]),
            &set(&[("GET", "/a")])
        );
        assert_eq!(score.precision(), Some(1.0));
        assert_eq!(score.recall(), Some(0.5));
        assert_eq!(score.disagreement.unrecovered, vec!["POST /b".to_owned()]);
    }

    #[test]
    fn a_undeclared_endpoint_lowers_precision_and_is_named() {
        let score = Score::of(
            &set(&[("GET", "/a")]),
            &set(&[("GET", "/a"), ("GET", "/z")])
        );
        assert_eq!(score.precision(), Some(0.5));
        assert_eq!(score.recall(), Some(1.0));
        assert_eq!(score.disagreement.undeclared, vec!["GET /z".to_owned()]);
    }

    #[test]
    fn a_method_mismatch_is_two_disagreements_not_one() {
        let score = Score::of(&set(&[("GET", "/a")]), &set(&[("POST", "/a")]));
        assert_eq!(score.counts.agreed, 0);
        assert_eq!(score.disagreement.unrecovered, vec!["GET /a".to_owned()]);
        assert_eq!(score.disagreement.undeclared, vec!["POST /a".to_owned()]);
    }

    #[test]
    fn an_absent_denominator_yields_no_figure_rather_than_zero() {
        let empty = set(&[]);
        assert_eq!(Score::of(&empty, &empty).precision(), None);
        assert_eq!(Score::of(&empty, &empty).recall(), None);
        assert_eq!(Score::of(&set(&[("GET", "/a")]), &empty).precision(), None);
        assert_eq!(Score::of(&empty, &set(&[("GET", "/a")])).recall(), None);
    }
}
