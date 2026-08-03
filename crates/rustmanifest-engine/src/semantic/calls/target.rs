// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cargo build targets of source files and the reachability between
//! them, the constraint that keeps resolution from binding a call to a
//! definition the caller never compiles with.

/// Cargo build target one source file compiles into.
///
/// Targets are separate compilation units: a library function cannot
/// call a function defined in the crate's `tests`, `benches`, or
/// `examples` directory, and two integration test files are two crates
/// that cannot call each other. Resolution reads this to discard
/// candidates that are unreachable by construction — a `Type::method`
/// or bare method name is only a match when the caller compiles
/// together with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Path of the crate directory the target belongs to.
    root: String,
    /// Which cargo target directory the file sits under.
    kind: TargetKind,
    /// The individual target inside a multi-target directory: the file
    /// stem of a top-level `tests/foo.rs`, or the directory name of a
    /// `tests/foo/main.rs`. `None` marks a shared module below the
    /// directory, which any target of that kind may mount.
    unit: Option<String>
}

/// Which cargo target directory a source file sits under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    /// Library or binary sources — everything under `src`, plus files
    /// outside a recognized target directory, which stay permissive.
    Library,
    /// Integration tests under `tests`.
    Tests,
    /// Benchmarks under `benches`.
    Benches,
    /// Examples under `examples`.
    Examples
}

impl Target {
    /// Derives the build target of one source path.
    #[must_use]
    pub fn of_path(path: &str) -> Self {
        let normalized = path.replace('\\', "/");
        let components: Vec<&str> = normalized.split('/').filter(|c| !c.is_empty()).collect();
        let marked = components.iter().enumerate().find_map(|(at, component)| {
            let kind = match *component {
                "src" => TargetKind::Library,
                "tests" => TargetKind::Tests,
                "benches" => TargetKind::Benches,
                "examples" => TargetKind::Examples,
                _ => return None
            };
            Some((at, kind))
        });
        let Some((at, kind)) = marked else {
            let root = components
                .len()
                .checked_sub(1)
                .map_or_else(String::new, |end| components[..end].join("/"));
            return Self {
                root,
                kind: TargetKind::Library,
                unit: None
            };
        };
        Self {
            root: components[..at].join("/"),
            kind,
            unit: unit_of(&components[at + 1..])
        }
    }

    /// Returns whether a caller in this target can reach a definition in
    /// `other`.
    ///
    /// The library of any crate is linkable from anywhere in the
    /// workspace. Every other target is private to itself: reachable
    /// only from the same crate directory, the same target kind, and the
    /// same unit — with a shared module below the directory, which
    /// several units may mount, admitted by either side.
    #[must_use]
    pub(super) fn reaches(&self, other: &Self) -> bool {
        if other.kind == TargetKind::Library {
            return true;
        }
        if self.kind != other.kind || self.root != other.root {
            return false;
        }
        match (&self.unit, &other.unit) {
            (Some(ours), Some(theirs)) => ours == theirs,
            _ => true
        }
    }
}

/// Names the individual target inside a cargo target directory: a
/// top-level `foo.rs` or a `foo/main.rs`. Anything deeper is a shared
/// module rather than a target of its own.
fn unit_of(rest: &[&str]) -> Option<String> {
    match rest {
        [only] => only.strip_suffix(".rs").map(ToOwned::to_owned),
        [directory, "main.rs"] => Some((*directory).to_owned()),
        _ => None
    }
}
