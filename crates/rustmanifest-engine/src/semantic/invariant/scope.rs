// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Module-path scoping: whether a module falls inside a declared list.
//!
//! Every invariant names the modules it constrains by path prefix. A
//! prefix covers the module it names and everything below it, and the
//! match is `::`-aligned so `crate::ledger` never swallows
//! `crate::ledgerx`.

/// Returns whether `module` equals one of `prefixes` or sits below one,
/// `::`-aligned.
pub(super) fn covered_by_any(module: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        module == prefix
            || (module.starts_with(prefix.as_str()) && module[prefix.len()..].starts_with("::"))
    })
}
