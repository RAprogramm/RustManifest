// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Resolution of `use` targets to modules of the same crate: absolute
//! `crate::`/`self::`/`super::` rewriting, Rust 2018 uniform paths, and
//! deepest-covering-module selection.

use super::ModuleNode;

/// Resolves `target`, as written inside the module at `from`, to a module
/// of the same crate.
///
/// The deepest module whose path covers the absolute target wins, so a
/// target naming an item resolves to the module defining it. External and
/// unresolvable targets yield `None`.
pub(in crate::semantic) fn resolved_module(
    modules: &[ModuleNode],
    from: usize,
    target: &str
) -> Option<usize> {
    let source = modules.get(from)?;
    let absolute = absolute_target(&source.path, target)
        .or_else(|| uniform_path_target(modules, from, target))?;
    let mut best: Option<(usize, usize)> = None;
    for (index, module) in modules.iter().enumerate() {
        if module.krate != source.krate || !path_covers(&module.path, &absolute) {
            continue;
        }
        let depth = module.path.len();
        if best.is_none_or(|(_, best_depth)| depth > best_depth) {
            best = Some((index, depth));
        }
    }
    Some(best?.0)
}

/// Returns true when `prefix` equals `path` or is a `::`-aligned prefix of
/// it.
pub(super) fn path_covers(prefix: &str, path: &str) -> bool {
    path == prefix || (path.starts_with(prefix) && path[prefix.len()..].starts_with("::"))
}

/// Rewrites `target` into an absolute `crate::` path relative to the module
/// path `source`, returning `None` for external or invalid targets.
pub(super) fn absolute_target(source: &str, target: &str) -> Option<String> {
    let mut segments = target.split("::");
    let head = segments.next()?;
    match head {
        "crate" => Some(target.to_owned()),
        "self" => Some(extended(source, segments)),
        "super" => {
            let mut ancestors: Vec<&str> = source.split("::").collect();
            ancestors.pop();
            let mut tail: Vec<&str> = Vec::new();
            for segment in segments {
                if tail.is_empty() && segment == "super" {
                    ancestors.pop();
                } else {
                    tail.push(segment);
                }
            }
            if ancestors.is_empty() {
                return None;
            }
            ancestors.extend(tail);
            Some(ancestors.join("::"))
        }
        _ => None
    }
}

/// Resolves a Rust 2018 uniform path — a `use` target whose head names a
/// child module of the current module rather than a crate — into an
/// absolute path, or [`None`] when no such child module exists.
///
/// The child-module check keeps external crates out: `use serde::…` only
/// becomes an in-crate path if the current module really declares a
/// `serde` submodule.
fn uniform_path_target(modules: &[ModuleNode], from: usize, target: &str) -> Option<String> {
    let source = modules.get(from)?;
    let head = target.split("::").next()?;
    let child = format!("{}::{head}", source.path);
    modules
        .iter()
        .any(|module| module.krate == source.krate && module.path == child)
        .then(|| extended(&source.path, target.split("::")))
}

/// Appends `segments` to the module path `base` with `::` separators.
fn extended<'a>(base: &str, segments: impl Iterator<Item = &'a str>) -> String {
    let mut path = base.to_owned();
    for segment in segments {
        path.push_str("::");
        path.push_str(segment);
    }
    path
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{absolute_target, path_covers};

    #[test]
    fn crate_targets_stay_absolute() {
        assert_eq!(
            absolute_target("crate::a", "crate::b::Item").as_deref(),
            Some("crate::b::Item")
        );
    }

    #[test]
    fn self_targets_extend_the_source_path() {
        assert_eq!(
            absolute_target("crate::a", "self::x").as_deref(),
            Some("crate::a::x")
        );
    }

    #[test]
    fn super_targets_pop_source_segments() {
        assert_eq!(
            absolute_target("crate::a::b", "super::c").as_deref(),
            Some("crate::a::c")
        );
        assert_eq!(
            absolute_target("crate::a::b", "super::super::c").as_deref(),
            Some("crate::c")
        );
    }

    #[test]
    fn super_above_the_crate_root_is_invalid() {
        assert_eq!(absolute_target("crate", "super::x"), None);
        assert_eq!(absolute_target("crate::a", "super::super::x"), None);
    }

    #[test]
    fn external_targets_are_not_resolved() {
        assert_eq!(absolute_target("crate::a", "std::fmt"), None);
        assert_eq!(absolute_target("crate::a", "serde::Serialize"), None);
    }

    #[test]
    fn coverage_is_segment_aligned() {
        assert!(path_covers("crate::a", "crate::a"));
        assert!(path_covers("crate::a", "crate::a::Item"));
        assert!(!path_covers("crate::a", "crate::ab"));
    }
}
