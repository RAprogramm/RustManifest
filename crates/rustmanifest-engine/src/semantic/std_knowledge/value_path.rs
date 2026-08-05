// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Paths of the standard library that stand for a value rather than name
//! one the program bound.
//!
//! A path is not a binding: `None` and the limit of an integer type are
//! written where a value is meant, and what they hold is fixed by the
//! library rather than by the body reading them. Reading them as names
//! this body never bound made them refusals, which said the origin was
//! unknown where it is in fact the most known kind there is.
//!
//! The table is deliberately narrow: only paths whose last segment the
//! library defines on a type this module names. A workspace constant of
//! the same spelling is not credited — the owner must match a primitive
//! this table lists, or the path is not read here at all.

/// Primitive types whose associated constants this table answers for.
const OWNERS: [&str; 16] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32",
    "f64", "char", "bool"
];

/// Associated constants those types define.
const CONSTANTS: [&str; 9] = [
    "MAX",
    "MIN",
    "BITS",
    "EPSILON",
    "INFINITY",
    "NEG_INFINITY",
    "NAN",
    "MIN_POSITIVE",
    "RADIX"
];

/// Unit variants of standard types, written bare or through their own
/// type, each standing for one value with no payload.
const UNIT_VARIANTS: [&str; 4] = ["None", "Less", "Equal", "Greater"];

/// The value a path stands for, named as written, or nothing where the
/// path is not one this table answers for.
///
/// The answer is the canonical spelling — `u32::MAX`, `None` — so two
/// sites writing the same constant through different imports name one
/// value.
pub(in crate::semantic) fn value_path(path: &syn::Path) -> Option<String> {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match segments.as_slice() {
        [name] if UNIT_VARIANTS.contains(&name.as_str()) => Some(name.clone()),
        [.., owner, name]
            if OWNERS.contains(&owner.as_str()) && CONSTANTS.contains(&name.as_str()) =>
        {
            Some(format!("{owner}::{name}"))
        }
        [.., owner, name] if UNIT_VARIANTS.contains(&name.as_str()) && variant_owner(owner) => {
            Some(name.clone())
        }
        _ => None
    }
}

/// Whether a segment names the standard type whose unit variants this
/// table lists.
fn variant_owner(owner: &str) -> bool {
    matches!(owner, "Option" | "Ordering")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::value_path;

    fn path(written: &str) -> syn::Path {
        syn::parse_str(written).unwrap()
    }

    #[test]
    fn a_bare_unit_variant_is_the_value_it_names() {
        assert_eq!(value_path(&path("None")).as_deref(), Some("None"));
        assert_eq!(
            value_path(&path("std::cmp::Ordering::Less")).as_deref(),
            Some("Less")
        );
    }

    #[test]
    fn an_associated_constant_keeps_the_type_that_defines_it() {
        assert_eq!(value_path(&path("u32::MAX")).as_deref(), Some("u32::MAX"));
        assert_eq!(
            value_path(&path("f64::EPSILON")).as_deref(),
            Some("f64::EPSILON")
        );
    }

    #[test]
    fn a_name_outside_the_table_is_not_answered_for() {
        assert!(value_path(&path("MAX")).is_none());
        assert!(value_path(&path("Config::MAX")).is_none());
        assert!(value_path(&path("crate::limits::DEFAULT")).is_none());
    }
}
