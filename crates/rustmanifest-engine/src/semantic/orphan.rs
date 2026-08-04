// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `orphan-file` check: a source file reachable from no crate root.
//!
//! The common cause is human, not dead code: the author wrote the module
//! but forgot the `mod name;` declaration, and then cannot understand why
//! the compiler does not see the handlers inside. The check fires only
//! when a plausible parent module file exists in the analyzed sources —
//! the modern directory-named file one level up first, the crate root for
//! direct `src/` children, legacy same-directory `mod.rs` last — so loose
//! fixture files in unstructured directories stay silent.
//!
//! The finding carries a file-addressed fix inserting `mod name;` into
//! the parent, marked maybe-incorrect: where a module is mounted is a
//! human decision, the engine only points at the most likely line.
//!
//! Humility guard: when the parent's text mentions `mod name` anywhere —
//! including inside macro invocations or `#[path]` arrangements the
//! resolver cannot see through — the declaration cannot be proven
//! missing, and the check stays silent.

use std::path::{Path, PathBuf};

use rustmanifest_schema::{Applicability, Fix, Range, TraceStep};

use super::{insight::Insight, workspace::Workspace};
use crate::source::Source;

/// File names that are roots or mount points themselves, never orphans.
const NEVER_ORPHAN: [&str; 4] = ["build.rs", "lib.rs", "main.rs", "mod.rs"];

/// Reports unreachable files that have a plausible parent module file.
pub(super) fn insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let graph = workspace.modules();
    let mut reachable = vec![false; sources.len()];
    for module in graph.modules() {
        if module.layout
            && let Some(flag) = reachable.get_mut(module.source)
        {
            *flag = true;
        }
    }
    let mut found = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        if reachable[index] {
            continue;
        }
        let path = source.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if NEVER_ORPHAN.contains(&file_name) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some((parent_index, parent_path)) = parent_module_file(sources, path) else {
            continue;
        };
        let parent_text = sources[parent_index].text();
        if parent_text.contains(&format!("mod {stem}")) {
            continue;
        }
        let insert_at = mod_insertion_offset(parent_text);
        let detail = format!(
            "file `{}` is reachable from no crate root — likely a forgotten `mod {stem};` in \
             `{}`",
            path.display(),
            parent_path.display()
        );
        let trace = vec![
            TraceStep {
                fact:   format!(
                    "`{}` belongs to no crate root's module tree",
                    path.display()
                ),
                reason: "the module graph reached it from no `mod` chain".to_owned()
            },
            TraceStep {
                fact:   format!(
                    "`{}` is its plausible parent module file",
                    parent_path.display()
                ),
                reason: "it exists in the analyzed sources and does not declare the module"
                    .to_owned()
            },
        ];
        let fixes = vec![Fix {
            file:          Some(parent_path),
            rename_to:     None,
            range:         Range {
                start: insert_at,
                end:   insert_at
            },
            replacement:   format!("mod {stem};\n"),
            applicability: Applicability::MaybeIncorrect
        }];
        found.push(
            Insight::new(
                index,
                Range {
                    start: 0, end: 0
                },
                detail,
                trace
            )
            .with_fixes(fixes)
        );
    }
    found
}

/// Finds the plausible parent module file of `path` among `sources`.
fn parent_module_file(sources: &[Source], path: &Path) -> Option<(usize, PathBuf)> {
    let dir = path.parent()?;
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let (Some(grand), Some(dir_name)) = (dir.parent(), dir.file_name()) {
        candidates.push(grand.join(format!("{}.rs", dir_name.to_string_lossy())));
        if dir_name == "src" {
            candidates.push(dir.join("lib.rs"));
            candidates.push(dir.join("main.rs"));
        }
    } else if dir.file_name().is_some_and(|name| name == "src") {
        candidates.push(dir.join("lib.rs"));
        candidates.push(dir.join("main.rs"));
    }
    candidates.push(dir.join("mod.rs"));
    for candidate in candidates {
        if candidate == path {
            continue;
        }
        if let Some(index) = sources
            .iter()
            .position(|source| source.path() == candidate.as_path())
        {
            return Some((index, candidate));
        }
    }
    None
}

/// Byte offset right after the last top-level `mod` declaration line, or
/// the start of the file when there is none.
fn mod_insertion_offset(text: &str) -> u32 {
    let mut offset = 0usize;
    let mut cursor = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
            offset = cursor + line.len();
        }
        cursor += line.len();
    }
    u32::try_from(offset).unwrap_or(0)
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

    use super::insights;
    use crate::{semantic::workspace::Workspace, source::Source};

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    #[test]
    fn forgotten_mod_is_reported_with_a_parent_addressed_fix() {
        let sources = [
            source("src/lib.rs", "mod ledger;\npub struct Wallet;\n"),
            source("src/ledger.rs", "pub fn post() {}\n"),
            source("src/handlers.rs", "pub fn on_click() {}\n")
        ];
        let found = insights(&Workspace::new(&sources));
        assert_eq!(found.len(), 1);
        assert!(found[0].detail.contains("forgotten `mod handlers;`"));
        assert_eq!(found[0].fixes.len(), 1);
        let fix = &found[0].fixes[0];
        assert_eq!(
            fix.file.as_deref(),
            Some(std::path::Path::new("src/lib.rs"))
        );
        assert_eq!(fix.replacement, "mod handlers;\n");
        assert_eq!(fix.range.start, 12);
    }

    #[test]
    fn declared_modules_pass() {
        let sources = [
            source("src/lib.rs", "mod handlers;\n"),
            source("src/handlers.rs", "pub fn on_click() {}\n")
        ];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn directory_modules_point_at_their_mod_rs() {
        let sources = [
            source("src/lib.rs", "mod net;\n"),
            source("src/net/mod.rs", "mod tcp;\n"),
            source("src/net/udp.rs", "pub fn bind() {}\n"),
            source("src/net/tcp.rs", "pub fn connect() {}\n")
        ];
        let found = insights(&Workspace::new(&sources));
        assert_eq!(found.len(), 1);
        assert!(found[0].detail.contains("`mod udp;`"));
        assert!(
            found[0].fixes[0]
                .file
                .as_deref()
                .is_some_and(|file| file.ends_with("net/mod.rs"))
        );
    }

    #[test]
    fn files_without_a_plausible_parent_stay_silent() {
        let sources = [
            source("src/lib.rs", "pub struct Wallet;\n"),
            source("fixtures/sample.rs", "pub fn probe() {}\n")
        ];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }
}
