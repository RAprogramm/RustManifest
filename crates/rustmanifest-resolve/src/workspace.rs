// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Walking a loaded workspace and recording what the resolver settles.
//!
//! Two things here were learned by measurement, not from documentation:
//!
//! - The database must be attached to the calling thread before any query, or
//!   the first one panics.
//! - Only files of the analyzed workspace are walked. Every dependency's
//!   sources are loaded too, and walking them turns seconds into minutes for
//!   answers about code nobody asked about.
//!
//! What is asked of each file is four questions, in [`typing`], [`naming`],
//! [`settling`] and [`expansion`]. The first two are the ones that matter
//! most to a reader downstream: with the type of every expression and the
//! item every written name reaches, nothing above has to infer either from
//! how the source spelled it.

mod declaring;
mod expansion;
mod load;
mod naming;
mod reading;
mod settling;
mod tree;
mod typing;

use std::path::Path;

use ra_ap_hir::{Semantics, attach_db};
use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::Resolution;
use tracing::debug;

use self::reading::Reading;
use crate::error::ResolveError;

/// Resolves every workspace file rooted at `root`.
///
/// A position the resolver cannot settle is left out; see the crate
/// documentation for why silence is the only honest encoding of that.
///
/// # Errors
///
/// Returns [`ResolveError::Load`] when the cargo workspace at `root` cannot
/// be loaded.
pub fn resolve(root: &Path) -> Result<Resolution, ResolveError> {
    let (database, files) = load::load(root)?;
    let inside = std::fs::canonicalize(root).map_or_else(
        |_| root.display().to_string(),
        |path| path.display().to_string()
    );
    let mut resolution = Resolution {
        resolver: format!("rustmanifest-resolve {}", env!("CARGO_PKG_VERSION")),
        ..Resolution::default()
    };
    attach_db(&database, || {
        let semantics = Semantics::new(&database);
        for (file, path) in files.iter() {
            let name = path.to_string();
            if !written_in_rust(&name) || !name.starts_with(&inside) {
                continue;
            }
            let parsed = semantics.parse_guess_edition(file);
            let reading = Reading {
                semantics: &semantics,
                database:  &database,
                file:      &name,
                root:      &inside
            };
            settle_file(reading, &parsed, &mut resolution);
        }
    });
    attach_db(&database, || {
        tree::settled(&database, &files, &inside, &mut resolution.modules);
    });
    resolution.settle();
    debug!(
        calls = resolution.calls.len(),
        fields = resolution.fields.len(),
        types = resolution.types.len(),
        paths = resolution.paths.len(),
        "workspace resolved"
    );
    Ok(resolution)
}

/// Asks every question of one parsed file.
fn settle_file(reading: Reading<'_, '_>, parsed: &ast::SourceFile, into: &mut Resolution) {
    typing::settled(
        reading,
        parsed,
        &mut into.types,
        &mut into.types_granted,
        &mut into.layouts
    );
    naming::settled(reading, parsed, &mut into.paths);
    declaring::settled(reading, parsed, &mut into.functions, &mut into.signatures);
    for call in parsed
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        if let Some(settled) = settling::settled_call(reading, &call) {
            into.calls.push(settled);
        }
    }
    for call in parsed
        .syntax()
        .descendants()
        .filter_map(ast::CallExpr::cast)
    {
        if let Some(settled) = settling::settled_free_call(reading, &call) {
            into.calls.push(settled);
        }
    }
    for field in parsed
        .syntax()
        .descendants()
        .filter_map(ast::FieldExpr::cast)
    {
        if let Some(settled) = settling::settled_field(reading, &field) {
            into.fields.push(settled);
        }
    }
    expansion::settled(reading, parsed, into);
}

/// Whether a path names a Rust source, by its extension.
fn written_in_rust(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}
