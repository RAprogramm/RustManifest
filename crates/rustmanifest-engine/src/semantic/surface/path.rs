// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Path arithmetic: the module path a source file implies, the path of an
//! item under its owner, and the URL path a mount prefix composes.

/// Module path a source file implies, from the first path component after
/// the last `src` directory. `lib.rs`, `main.rs` and `mod.rs` name their
/// parent rather than themselves.
pub(super) fn module_path(path: &std::path::Path) -> Vec<String> {
    let components: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let start = components
        .iter()
        .rposition(|component| component == "src")
        .map_or(0, |at| at + 1);
    let last = components.len().saturating_sub(1);
    let mut out = Vec::new();
    for (offset, component) in components.iter().enumerate().skip(start) {
        if offset < last {
            out.push(component.clone());
            continue;
        }
        let stem = component
            .rsplit_once('.')
            .map_or(component.as_str(), |(stem, _)| stem);
        if !matches!(stem, "lib" | "main" | "mod") {
            out.push(stem.to_owned());
        }
    }
    out
}

/// Appends one segment to a path.
pub(super) fn appended(scope: &[String], segment: &str) -> Vec<String> {
    let mut out = scope.to_vec();
    out.push(segment.to_owned());
    out
}

/// Joins a prefix and a path without doubling or dropping separators.
pub(super) fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        return path.to_owned();
    }
    if path.is_empty() || path == "/" {
        return prefix.to_owned();
    }
    format!(
        "{}/{}",
        prefix.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}
