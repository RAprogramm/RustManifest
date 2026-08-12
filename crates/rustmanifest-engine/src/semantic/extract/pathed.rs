// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The `#[path]` mount: a file another file mounts by a written path
//! is mod-rs style, so its children resolve beside the file itself —
//! flat in its own directory — never in a folder named after its stem.
//!
//! The corpus judge caught the difference: a module file planted under
//! the stem folder of a `#[path]`-mounted parent is a file the
//! resolver never looks at, and the build refuses the mount whole.

use std::path::{Path, PathBuf};

use crate::source::Source;

/// Whether any source mounts the file at `path` with a written
/// top-level `#[path = "..."] mod ...;` declaration.
pub(super) fn mounted_by_path(sources: &[Source], path: &Path) -> bool {
    sources.iter().any(|source| {
        let Ok(parsed) = syn::parse_file(source.text()) else {
            return false;
        };
        let Some(base) = source.path().parent() else {
            return false;
        };
        parsed
            .items
            .iter()
            .any(|item| pathed_target(item, base).is_some_and(|target| target == path))
    })
}

/// The file a top-level `#[path = "..."] mod ...;` declaration mounts,
/// resolved beside the declaring file.
fn pathed_target(item: &syn::Item, base: &Path) -> Option<PathBuf> {
    let syn::Item::Mod(module) = item else {
        return None;
    };
    module.attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(pair) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(written) = &pair.value else {
            return None;
        };
        let syn::Lit::Str(text) = &written.lit else {
            return None;
        };
        Some(base.join(text.value()))
    })
}
