// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The `--unresolved` listing of unresolved `use` targets.

/// Renders the `--unresolved` listing: one line per unresolved target,
/// most frequent first, ties broken alphabetically.
pub(super) fn unresolved_lines(targets: &std::collections::BTreeMap<String, u64>) -> Vec<String> {
    let mut ordered: Vec<(&String, &u64)> = targets.iter().collect();
    ordered.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
    ordered
        .into_iter()
        .map(|(target, count)| format!("unresolved          x{count:<8} {target}"))
        .collect()
}
