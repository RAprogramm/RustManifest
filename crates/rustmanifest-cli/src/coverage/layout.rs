// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Column layout shared by every rendered coverage line.

/// Formats one report line: a padded label column, a padded value column,
/// and a free-form tail, without trailing whitespace.
pub(super) fn line(label: &str, value: &str, tail: &str) -> String {
    format!("{label:<20}{value:<10}{tail}")
        .trim_end()
        .to_owned()
}
