// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! SCIP symbol grammar: which symbols name a callable, and how one is
//! read into the engine's callee shape.
//!
//! A SCIP symbol is a space-separated header followed by a descriptor
//! path. This module owns every rule for taking that text apart —
//! function suffix, crate header token, descriptor segments, `impl`
//! disambiguators, backtick-escaped generic owners — so the rest of the
//! layer works with a decoded [`Callee`] instead of raw symbol strings.

/// One name-resolved callee read from the index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Callee {
    /// Short qualified name in the engine's shape.
    pub(super) short:      String,
    /// Crate the symbol belongs to, from the symbol header.
    pub(super) crate_name: String,
    /// Full descriptor path: modules, owner type, method.
    pub(super) segments:   Vec<String>
}

/// Returns whether a symbol names a function or method.
pub(super) fn function_symbol(symbol: &str) -> bool {
    symbol.ends_with("().")
}

/// Parses a SCIP function symbol into the engine's callee shape: the
/// short qualified name, the owning crate from the symbol header, and
/// the full descriptor path for collision settling.
///
/// Descriptors start after the fourth space-separated header token;
/// `impl` owners resolve through the `[Type]` disambiguator, and
/// backtick-escaped generic owners drop their arguments.
pub(super) fn callee(symbol: &str) -> Callee {
    let crate_name = symbol.split(' ').nth(2).unwrap_or_default().to_owned();
    let descriptor = symbol
        .splitn(5, ' ')
        .nth(4)
        .unwrap_or(symbol)
        .trim_end_matches("().");
    let mut raw: Vec<&str> = descriptor
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let last = raw.pop().unwrap_or_default();
    let mut segments: Vec<String> = raw.into_iter().map(strip_generics).collect();
    let (owner, method) = owner_method(last);
    if !owner.is_empty() {
        segments.push(owner.clone());
    }
    segments.push(method.clone());
    let short = if owner.is_empty() {
        method
    } else {
        format!("{owner}::{method}")
    };
    Callee {
        short,
        crate_name,
        segments
    }
}

/// Splits one descriptor segment into `(owner type, method name)`, the
/// owner empty for free functions.
fn owner_method(segment: &str) -> (String, String) {
    let Some((owner, method)) = segment.split_once('#') else {
        let (_, bare) = split_brackets(segment);
        return (String::new(), strip_generics(&bare));
    };
    let (bracketed, bare) = split_brackets(method);
    let owner = strip_generics(owner);
    let owner = if owner == "impl" || owner.is_empty() {
        bracketed
            .map_or_default(|inner| strip_generics(&inner))
    } else {
        owner
    };
    (owner, bare)
}

/// Renders a SCIP function symbol as the engine's qualified name shape.
pub(super) fn qualified_name(symbol: &str) -> String {
    callee(symbol).short
}

/// Splits leading `[..]` disambiguators off a descriptor name, keeping
/// the first one — the self type of an `impl` block.
fn split_brackets(name: &str) -> (Option<String>, String) {
    let mut rest = name;
    let mut first = None;
    while rest.starts_with('[') {
        let Some(close) = rest.find(']') else {
            break;
        };
        if first.is_none() {
            first = Some(rest[1..close].to_string());
        }
        rest = &rest[close + 1..];
    }
    (first, rest.to_string())
}

/// Drops backtick escaping and generic arguments from a type name.
fn strip_generics(name: &str) -> String {
    let trimmed = name.trim_matches('`');
    trimmed
        .find('<')
        .map_or_else(|| trimmed.to_string(), |open| trimmed[..open].to_string())
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
