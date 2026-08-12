// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The compiler's own module tree of the analyzed workspace.
//!
//! Walked from each crate root down through the children the compiler
//! knows, so a module mounted by a `path` attribute, one behind a
//! configuration flag, and one whose file does not match its name are all
//! placed where the compiler places them rather than where a file-name
//! reading would guess.

use ra_ap_hir::{Crate, Module};
use ra_ap_ide_db::RootDatabase;
use ra_ap_vfs::Vfs;
use rustmanifest_schema::ResolvedModule;

/// Records every module of every crate rooted inside `root`.
pub(super) fn settled(
    database: &RootDatabase,
    files: &Vfs,
    root: &str,
    into: &mut Vec<ResolvedModule>
) {
    for krate in Crate::all(database) {
        let Some(named) = krate.display_name(database) else {
            continue;
        };
        walk(
            database,
            files,
            root,
            krate.root_module(database),
            &named.to_string(),
            None,
            into
        );
    }
}

/// Records one module and every module below it.
fn walk(
    database: &RootDatabase,
    files: &Vfs,
    root: &str,
    module: Module,
    names: &str,
    parent: Option<&str>,
    into: &mut Vec<ResolvedModule>
) {
    if let Some(file) = written_in(database, files, root, module) {
        into.push(ResolvedModule {
            names: names.to_owned(),
            file,
            crate_root: module.is_crate_root(database),
            parent: parent.map(ToOwned::to_owned)
        });
    }
    for child in module.children(database) {
        let Some(name) = child.name(database) else {
            continue;
        };
        let below = format!("{names}::{}", name.as_str());
        walk(database, files, root, child, &below, Some(names), into);
    }
}

/// The workspace-relative file a module's body is written in, or `None`
/// for a module outside the analyzed tree.
fn written_in(
    database: &RootDatabase,
    files: &Vfs,
    root: &str,
    module: Module
) -> Option<std::path::PathBuf> {
    let held = module.definition_source_file_id(database).file_id()?;
    let named = files
        .file_path(held.file_id(database))
        .as_path()?
        .to_string();
    let relative = std::path::Path::new(&named)
        .strip_prefix(root)
        .ok()?
        .to_path_buf();
    Some(relative)
}
