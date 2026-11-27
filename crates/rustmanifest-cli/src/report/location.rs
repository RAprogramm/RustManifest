// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where a reported line points, said as short as the analyzed root
//! allows.

/// Shortens a path against the analyzed root so a line stays readable.
pub(super) fn relative(path: &str, root: &std::path::Path) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let root = root.trim_end_matches('/');
    if root.is_empty() || root == "." {
        return path.trim_start_matches("./").to_owned();
    }
    path.strip_prefix(root).map_or_else(
        || path.to_owned(),
        |rest| rest.trim_start_matches('/').to_owned(),
    )
}
