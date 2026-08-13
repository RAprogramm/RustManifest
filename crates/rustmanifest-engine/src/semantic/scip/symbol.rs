// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! SCIP symbol binding into the engine's callee shape.
//!
//! Parses SCIP symbols through [`scip::symbol::parse_symbol`], extracting
//! the short qualified name, owning crate, and full descriptor path.

use scip::symbol::parse_symbol;
use scip::types::descriptor;

/// One name-resolved callee read from the index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Callee {
    /// Short qualified name in the engine's shape.
    pub(super) short:      String,
    /// Crate the symbol belongs to, from the symbol package name.
    pub(super) crate_name: String,
    /// Full descriptor path: modules, owner type, method.
    pub(super) segments:   Vec<String>
}

/// Returns whether a symbol names a function or method.
pub(super) fn function_symbol(symbol: &str) -> bool {
    parse_symbol(symbol)
        .ok()
        .and_then(|parsed| {
            parsed.descriptors.last().and_then(|desc| {
                desc.suffix.enum_value().ok().map(|suffix| {
                    matches!(suffix, descriptor::Suffix::Method | descriptor::Suffix::Macro)
                })
            })
        })
        .unwrap_or(false)
}

/// Parses a SCIP function symbol into the engine's callee shape: the
/// short qualified name, the owning crate, and the full descriptor path.
pub(super) fn callee(symbol: &str) -> Callee {
    let parsed = parse_symbol(symbol).unwrap_or_default();
    let crate_name = parsed
        .package
        .as_ref()
        .map(|pkg| pkg.name.clone())
        .unwrap_or_default();
    let segments: Vec<String> = parsed
        .descriptors
        .iter()
        .filter_map(|desc| {
            if desc.name.is_empty() {
                return None;
            }
            let name_before_generics = if desc.name.contains('<') {
                desc.name.split('<').next().unwrap_or("")
            } else {
                &desc.name
            };
            let name = name_before_generics.trim_matches('`');
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect();
    let short = if segments.len() >= 2 {
        format!("{}::{}", segments[segments.len() - 2], segments[segments.len() - 1])
    } else if let Some(last) = segments.last() {
        last.clone()
    } else {
        String::new()
    };
    Callee {
        short,
        crate_name,
        segments
    }
}

/// Renders a SCIP function symbol as the engine's qualified name shape.
pub(super) fn qualified_name(symbol: &str) -> String {
    callee(symbol).short
}

#[cfg(test)]
mod tests {
    use super::qualified_name;

    #[test]
    fn symbols_render_as_engine_qualified_names() {
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 app/Wallet#credit()."),
            "Wallet::credit"
        );
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 error/impl#[Error]other()."),
            "Error::other"
        );
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 lib/`Criterion<M>`#bench()."),
            "Criterion::bench"
        );
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 util/helper()."),
            "helper"
        );
    }
}
