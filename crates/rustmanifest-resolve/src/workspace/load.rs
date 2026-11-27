// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Loading a cargo workspace so that every question below has an answer.
//!
//! Two settings here were learned by measurement, not from documentation,
//! and each one is load-bearing:
//!
//! - The standard-library sources must be asked for explicitly. Without them a
//!   call into the library resolves to nothing and its type reads as unknown —
//!   on a small program that is the difference between every call answered and
//!   one in seven.
//! - The test configuration must be turned on, or every `#[cfg(test)]` module
//!   is inactive code the resolver answers nothing about — which on a
//!   well-tested workspace is most of the file.

use std::path::Path;

use masterror::AppError;
use ra_ap_cfg::{CfgAtom, CfgDiff};
use ra_ap_hir::Symbol;
use ra_ap_ide_db::RootDatabase;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, CfgOverrides, RustLibSource};

use crate::error::ResolveError;

/// Loads the workspace at `root` with the standard-library sources attached.
///
/// # Errors
///
/// Returns an [`AppError`] when the workspace cannot be loaded.
pub(super) fn load(root: &Path) -> Result<(RootDatabase, ra_ap_vfs::Vfs), AppError> {
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
        }
        .into())
    }
}
