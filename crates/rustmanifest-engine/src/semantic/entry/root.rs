// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which files may declare a program, read from the path alone.

/// Returns whether a path names a file that compiles into a binary.
///
/// Position is part of the rule and not only the directory name: cargo
/// builds `<pkg>/src/bin/*.rs` and `<pkg>/examples/*.rs`, so a `bin`
/// directory anywhere but directly under `src`, and an `examples`
/// directory inside `src`, are ordinary module directories.
pub(super) fn binary_root(path: &str) -> bool {
    let components: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let last = ancestor(&components, 1);
    if !std::path::Path::new(last)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
    {
        return false;
    }
    let parent = ancestor(&components, 2);
    let grandparent = ancestor(&components, 3);
    match parent {
        "src" => last == "main.rs",
        "bin" => grandparent == "src",
        "examples" => grandparent != "src",
        _ if last == "main.rs" => match grandparent {
            "bin" => ancestor(&components, 4) == "src",
            "examples" => ancestor(&components, 4) != "src",
            _ => false
        },
        _ => false
    }
}

/// The component `back` positions from the end, or the empty string when
/// the path is shorter than that.
fn ancestor<'path>(components: &[&'path str], back: usize) -> &'path str {
    components
        .len()
        .checked_sub(back)
        .and_then(|index| components.get(index))
        .copied()
        .unwrap_or_default()
}
