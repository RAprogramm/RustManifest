// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The rename-module operation: one provable fix bundle that renames a
//! module everywhere — the `mod` declaration, the file on disk, and every
//! path that mentions the module — so the human touches no code at all.
//!
//! Safety gates, all or nothing: exactly one `mod <old>;` declaration
//! exists in the workspace; no other module shares the old name; the
//! module is a flat named file (`old.rs`) next to its declaring file —
//! otherwise no fixes are produced. Path mentions are rewritten wherever
//! a path segment equals the old name: `use` trees and qualified
//! expression paths. Byte edits apply before the file rename.

use std::path::PathBuf;

use rustmanifest_schema::{Applicability, Fix, Range};
use syn::visit::Visit;

use crate::source::Source;

/// Builds the full rename bundle for module `old` -> `new`, or returns
/// an empty list when safety cannot be proven.
#[must_use]
pub fn rename_module_fixes(sources: &[Source], old: &str, new: &str) -> Vec<Fix> {
    if old == new || new.is_empty() || !new.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Vec::new();
    }
    let mut scan = Scan {
        old,
        new,
        current: PathBuf::new(),
        declarations: Vec::new(),
        edits: Vec::new()
    };
    for source in sources {
        let Some(file) = source.tree() else {
            continue;
        };
        scan.current = source.path().to_path_buf();
        scan.visit_file(file);
    }
    let [(declaring_file, inline)] = scan.declarations.as_slice() else {
        return Vec::new();
    };
    if *inline {
        return scan.edits;
    }
    let dir = declaring_file
        .parent()
        .map_or_else(PathBuf::new, PathBuf::from);
    let old_path = dir.join(format!("{old}.rs"));
    if !sources.iter().any(|source| source.path() == old_path) {
        return Vec::new();
    }
    scan.edits.push(Fix {
        file:          Some(old_path),
        rename_to:     Some(dir.join(format!("{new}.rs"))),
        range:         Range {
            start: 0, end: 0
        },
        replacement:   String::new(),
        applicability: Applicability::MachineApplicable
    });
    scan.edits
}

/// Path-segment scanner producing byte edits for every mention.
struct Scan<'names> {
    /// Old module name.
    old:          &'names str,
    /// New module name.
    new:          &'names str,
    /// Path of the source currently visited.
    current:      PathBuf,
    /// Declaring files of `mod old`, with an inline-body marker.
    declarations: Vec<(PathBuf, bool)>,
    /// Accumulated byte edits.
    edits:        Vec<Fix>
}

impl Scan<'_> {
    /// Records one segment replacement at the ident's span.
    fn replace_ident(&mut self, ident: &syn::Ident) {
        let bytes = ident.span().byte_range();
        self.edits.push(Fix {
            file:          Some(self.current.clone()),
            rename_to:     None,
            range:         Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            },
            replacement:   self.new.to_owned(),
            applicability: Applicability::MachineApplicable
        });
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.ident == self.old {
            self.declarations
                .push((self.current.clone(), item.content.is_some()));
            self.replace_ident(&item.ident);
        }
        syn::visit::visit_item_mod(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        for segment in &path.segments {
            if segment.ident == self.old {
                self.replace_ident(&segment.ident);
            }
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_use_tree(&mut self, tree: &'ast syn::UseTree) {
        match tree {
            syn::UseTree::Path(segment) if segment.ident == self.old => {
                self.replace_ident(&segment.ident);
            }
            syn::UseTree::Name(name) if name.ident == self.old => {
                self.replace_ident(&name.ident);
            }
            _ => {}
        }
        syn::visit::visit_use_tree(self, tree);
    }
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

    use super::rename_module_fixes;
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    #[test]
    fn rename_bundle_covers_declaration_uses_paths_and_the_file() {
        let sources = [
            source(
                "src/lib.rs",
                "mod ledger;\nuse ledger::post;\npub fn run() { \
                    ledger::audit(); post(); }"
            ),
            source("src/ledger.rs", "pub fn post() {}\npub fn audit() {}")
        ];
        let fixes = rename_module_fixes(&sources, "ledger", "accounting");
        let renames: Vec<_> = fixes.iter().filter(|fix| fix.rename_to.is_some()).collect();
        let edits: Vec<_> = fixes.iter().filter(|fix| fix.rename_to.is_none()).collect();
        assert_eq!(renames.len(), 1);
        assert!(
            renames[0]
                .rename_to
                .as_deref()
                .is_some_and(|to| to.ends_with("accounting.rs"))
        );
        assert_eq!(edits.len(), 3);
        assert!(edits.iter().all(|fix| fix.replacement == "accounting"));
    }

    #[test]
    fn ambiguous_module_names_produce_nothing() {
        let sources = [
            source("src/lib.rs", "mod ledger; mod extra;"),
            source("src/ledger.rs", "pub fn post() {}"),
            source("src/extra.rs", "mod ledger { pub fn other() {} }")
        ];
        assert_eq!(
            rename_module_fixes(&sources, "ledger", "accounting"),
            [] as [rustmanifest_schema::Fix; 0]
        );
    }

    #[test]
    fn invalid_new_names_produce_nothing() {
        let sources = [
            source("src/lib.rs", "mod ledger;"),
            source("src/ledger.rs", "pub fn post() {}")
        ];
        assert_eq!(
            rename_module_fixes(&sources, "ledger", "bad-name"),
            [] as [rustmanifest_schema::Fix; 0]
        );
        assert_eq!(
            rename_module_fixes(&sources, "ledger", "ledger"),
            [] as [rustmanifest_schema::Fix; 0]
        );
    }

    #[test]
    fn inline_modules_rename_without_a_file_operation() {
        let sources = [source(
            "src/lib.rs",
            "mod ledger { pub fn post() {} }\npub fn run() { ledger::post(); }"
        )];
        let fixes = rename_module_fixes(&sources, "ledger", "accounting");
        assert_ne!(fixes, [] as [rustmanifest_schema::Fix; 0]);
        assert!(fixes.iter().all(|fix| fix.rename_to.is_none()));
    }
}
