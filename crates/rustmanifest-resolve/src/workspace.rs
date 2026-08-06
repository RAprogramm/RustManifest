// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Loading a cargo workspace and reading what the resolver settled.
//!
//! Three things here were learned by measurement, not from documentation,
//! and each one is load-bearing:
//!
//! - The database must be attached to the calling thread before any query, or
//!   the first one panics.
//! - The standard-library sources must be asked for explicitly. Without them a
//!   call into the library resolves to nothing and its type reads as unknown —
//!   on a small program that is the difference between every call answered and
//!   one in seven.
//! - The test configuration must be turned on, or every `#[cfg(test)]` module
//!   is inactive code the resolver answers nothing about — which on a
//!   well-tested workspace is most of the file.
//! - Only files of the analyzed workspace are walked. Every dependency's
//!   sources are loaded too, and walking them turns seconds into minutes for
//!   answers about code nobody asked about.

use std::path::Path;

use ra_ap_cfg::{CfgAtom, CfgDiff};
use ra_ap_hir::{DisplayTarget, HasCrate, HirDisplay, Semantics, Symbol, attach_db};
use ra_ap_ide_db::RootDatabase;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, CfgOverrides, RustLibSource};
use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::{Location, Range, Resolution, ResolvedCall, ResolvedField};
use tracing::debug;

use crate::error::ResolveError;

/// Resolves every method call written in the workspace rooted at `root`.
///
/// A call the resolver cannot settle is left out; see the crate
/// documentation for why silence is the only honest encoding of that.
///
/// # Errors
///
/// Returns [`ResolveError::Load`] when the cargo workspace at `root` cannot
/// be loaded.
pub fn resolve(root: &Path) -> Result<Resolution, ResolveError> {
    let (database, files) = load(root)?;
    let inside = std::fs::canonicalize(root).map_or_else(
        |_| root.display().to_string(),
        |path| path.display().to_string()
    );
    let mut resolution = Resolution {
        resolver: format!("rustmanifest-resolve {}", env!("CARGO_PKG_VERSION")),
        calls:    Vec::new(),
        fields:   Vec::new()
    };
    attach_db(&database, || {
        let semantics = Semantics::new(&database);
        for (file, path) in files.iter() {
            let name = path.to_string();
            if !written_in_rust(&name) || !name.starts_with(&inside) {
                continue;
            }
            let parsed = semantics.parse_guess_edition(file);
            for call in parsed
                .syntax()
                .descendants()
                .filter_map(ast::MethodCallExpr::cast)
            {
                if let Some(settled) = settled_call(&semantics, &database, &name, &call) {
                    resolution.calls.push(settled);
                }
            }
            for field in parsed
                .syntax()
                .descendants()
                .filter_map(ast::FieldExpr::cast)
            {
                if let Some(settled) = settled_field(&semantics, &database, &name, &field) {
                    resolution.fields.push(settled);
                }
            }
            settled_inside_macros(&semantics, &database, &name, &parsed, &mut resolution);
        }
    });
    resolution.settle();
    debug!(calls = resolution.calls.len(), "workspace resolved");
    Ok(resolution)
}

/// Whether a path names a Rust source, by its extension.
fn written_in_rust(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

/// Loads the workspace with the standard-library sources attached.
fn load(root: &Path) -> Result<(RootDatabase, ra_ap_vfs::Vfs), ResolveError> {
    let cargo = CargoConfig {
        sysroot: Some(RustLibSource::Discover),
        cfg_overrides: CfgOverrides {
            global: CfgDiff::new(vec![CfgAtom::Flag(Symbol::intern("test"))], Vec::new()),
            ..CfgOverrides::default()
        },
        ..CargoConfig::default()
    };
    let load = LoadCargoConfig {
        load_out_dirs_from_check: false,
        with_proc_macro_server:   ProcMacroServerChoice::None,
        prefill_caches:           false,
        num_worker_threads:       1,
        proc_macro_processes:     0
    };
    match load_workspace_at(root, &cargo, &load, &|_| {}) {
        Ok((database, files, _macros)) => Ok((database, files)),
        Err(source) => Err(ResolveError::Load {
            path:   root.to_path_buf(),
            reason: source.to_string()
        })
    }
}

/// Records every field read and every method call written inside a macro
/// argument, mapped back to the range the source wrote it at.
///
/// A field inside a macro argument is a token to a walk over the source
/// tree and an expression only after expansion — which is where the
/// positions a format string prints live. The expansion of a print
/// carries another macro inside it, so one level is not enough and the
/// walk goes two: far enough for the argument list, and bounded so a
/// macro defined in terms of itself cannot run the walk away.
fn settled_inside_macros(
    semantics: &Semantics<'_, RootDatabase>,
    database: &RootDatabase,
    file: &str,
    parsed: &ast::SourceFile,
    resolution: &mut Resolution
) {
    for invocation in parsed
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
    {
        let Some(expanded) = semantics.expand_macro_call(&invocation) else {
            continue;
        };
        let mut trees = vec![expanded.value.clone()];
        for inner in expanded
            .value
            .descendants()
            .filter_map(ast::MacroCall::cast)
        {
            if let Some(deeper) = semantics.expand_macro_call(&inner) {
                trees.push(deeper.value);
            }
        }
        for tree in trees {
            for field in tree.descendants().filter_map(ast::FieldExpr::cast) {
                let Some(mut settled) = settled_field(semantics, database, file, &field) else {
                    continue;
                };
                let written = semantics.original_range(field.syntax()).range;
                settled.at.range = Range {
                    start: u32::from(written.start()),
                    end:   u32::from(written.end())
                };
                resolution.fields.push(settled);
            }
            for call in tree.descendants().filter_map(ast::MethodCallExpr::cast) {
                let Some(mut settled) = settled_call(semantics, database, file, &call) else {
                    continue;
                };
                let written = semantics.original_range(call.syntax()).range;
                settled.at.range = Range {
                    start: u32::from(written.start()),
                    end:   u32::from(written.end())
                };
                resolution.calls.push(settled);
            }
        }
    }
}

/// Reads one field access, answering `None` where the resolver settled
/// nothing.
///
/// Both types are asked for: the one holding the field, which is what
/// the engine names a field by, and the one the field itself has, which
/// is what a reading standing on it needs. Either coming back unknown
/// leaves the position out, because a half-answer here would be read as
/// a whole one.
fn settled_field(
    semantics: &Semantics<'_, RootDatabase>,
    database: &RootDatabase,
    file: &str,
    field: &ast::FieldExpr
) -> Option<ResolvedField> {
    let base = field.expr()?;
    let owner_type = semantics.type_of_expr(&base)?.original();
    let target = DisplayTarget::from_crate(database, owner_type.krate(database).into());
    let owner = owner_type.display(database, target).to_string();
    let produced = semantics
        .type_of_expr(&ast::Expr::FieldExpr(field.clone()))?
        .original()
        .display(database, target)
        .to_string();
    if owner.contains("{unknown}") || produced.contains("{unknown}") {
        return None;
    }
    let span = field.syntax().text_range();
    Some(ResolvedField {
        at: Location {
            file:  std::path::PathBuf::from(file),
            range: Range {
                start: u32::from(span.start()),
                end:   u32::from(span.end())
            }
        },
        owner,
        produced
    })
}

/// Reads one method call, answering `None` where the resolver settled
/// nothing.
fn settled_call(
    semantics: &Semantics<'_, RootDatabase>,
    database: &RootDatabase,
    file: &str,
    call: &ast::MethodCallExpr
) -> Option<ResolvedCall> {
    let function = semantics.resolve_method_call(call)?;
    let span = call.syntax().text_range();
    let target = DisplayTarget::from_crate(database, function.krate(database).into());
    let returns = function
        .ret_type(database)
        .display(database, target)
        .to_string();
    if returns.contains("{unknown}") {
        return None;
    }
    Some(ResolvedCall {
        at: Location {
            file:  std::path::PathBuf::from(file),
            range: Range {
                start: u32::from(span.start()),
                end:   u32::from(span.end())
            }
        },
        callee: function.name(database).as_str().to_owned(),
        returns,
        receiver: call
            .receiver()
            .and_then(|expression| semantics.type_of_expr(&expression))
            .map(|typed| typed.original().display(database, target).to_string())
    })
}
