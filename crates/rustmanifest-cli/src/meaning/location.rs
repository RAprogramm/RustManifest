// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where a claim sits, said as short as the analyzed root allows.

/// Shortens a path against the analyzed root so the report stays
/// readable; a path outside the root is left as written.
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::Path;

    use super::relative;

    #[test]
    fn paths_shorten_against_the_analyzed_root() {
        assert_eq!(
            relative("/work/demo/src/main.rs", Path::new("/work/demo")),
            "src/main.rs"
        );
        assert_eq!(relative("src/main.rs", Path::new(".")), "src/main.rs");
        assert_eq!(
            relative("/elsewhere/main.rs", Path::new("/work/demo")),
            "/elsewhere/main.rs"
        );
    }
}
