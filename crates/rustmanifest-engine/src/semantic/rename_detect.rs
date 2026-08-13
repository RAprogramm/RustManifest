// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Rename detection between two snapshots of a workspace.
//!
//! A rename is proven, not guessed: a function name that disappeared and
//! a name that appeared are paired only when their name-independent
//! behavior fingerprints match uniquely on both sides. Ambiguity —
//! several vanished or several new functions sharing one fingerprint —
//! produces no pair, so a diff never mislabels a rewrite as a rename.

use std::collections::BTreeMap;

use super::{behavior::FunctionFacts, fingerprint::behavior_fingerprint};

/// One proven rename between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// Qualified name in the old snapshot.
    pub from: String,
    /// Qualified name in the new snapshot.
    pub to:   String
}

/// Detects renames between `before` and `after` behavioral snapshots.
#[must_use]
pub fn detect_renames(before: &[FunctionFacts], after: &[FunctionFacts]) -> Vec<Rename> {
    let old_names: BTreeMap<&str, u64> = before
        .iter()
        .map(|fact| (fact.function.as_str(), behavior_fingerprint(fact)))
        .collect();
    let new_names: BTreeMap<&str, u64> = after
        .iter()
        .map(|fact| (fact.function.as_str(), behavior_fingerprint(fact)))
        .collect();
    let mut vanished: BTreeMap<u64, Vec<&str>> = BTreeMap::new();
    for (name, print) in &old_names {
        if !new_names.contains_key(name) {
            vanished.entry(*print).or_default().push(name);
        }
    }
    let mut appeared: BTreeMap<u64, Vec<&str>> = BTreeMap::new();
    for (name, print) in &new_names {
        if !old_names.contains_key(name) {
            appeared.entry(*print).or_default().push(name);
        }
    }
    let mut renames = Vec::new();
    for (print, old) in &vanished {
        if let ([from], Some([to])) = (old.as_slice(), appeared.get(print).map(Vec::as_slice)) {
            renames.push(Rename {
                from: (*from).to_owned(),
                to:   (*to).to_owned()
            });
        }
    }
    renames
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

    use super::detect_renames;
    use crate::source::Source;

    fn facts(text: &str) -> Vec<crate::semantic::FunctionFacts> {
        {
            let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
            let settled = crate::semantic::settling::settled_over(&sources);
            crate::semantic::behavior_with(&sources, Some(&settled))
        }
    }

    #[test]
    fn pure_rename_is_proven() {
        let before = facts("struct W { b: i64 }\nfn credit(w: &mut W, a: i64) { w.b += a; }");
        let after = facts("struct W { b: i64 }\nfn deposit(w: &mut W, a: i64) { w.b += a; }");
        let renames = detect_renames(&before, &after);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].from, "credit");
        assert_eq!(renames[0].to, "deposit");
    }

    #[test]
    fn fact_visible_rewrite_is_not_a_rename() {
        let before = facts("struct W { b: i64 }\nfn credit(w: &mut W, a: i64) { w.b += a; }");
        let after = facts(
            "struct W { b: i64 }\nfn deposit(w: &mut W, a: i64) { w.b += a; \
             println!(\"{a}\"); }"
        );
        assert!(detect_renames(&before, &after).is_empty());
    }

    #[test]
    fn ambiguous_twins_produce_no_pair() {
        let before = facts("fn one(v: u64) -> u64 { v }\nfn two(v: u64) -> u64 { v }");
        let after = facts("fn other(v: u64) -> u64 { v }\nfn another(v: u64) -> u64 { v }");
        assert!(detect_renames(&before, &after).is_empty());
    }

    #[test]
    fn unchanged_snapshots_report_nothing() {
        let text = "fn keep(v: u64) -> u64 { v }";
        assert!(detect_renames(&facts(text), &facts(text)).is_empty());
    }
}
