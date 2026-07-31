// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `constant-encapsulation` check: constants belong to the types that use
//! them, not to module scope (`STRUCTURE.md` §8).
//!
//! Only module-level `pub const` items are reported. Private and
//! `pub(crate)`-restricted module consts pass, as do associated consts
//! inside impl blocks — those are exactly where constants should live.

use super::{Occurrence, flattened};

/// Reports module-level `pub const` items.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        let syn::Item::Const(constant) = item else {
            continue;
        };
        if !matches!(constant.vis, syn::Visibility::Public(_)) {
            continue;
        }
        found.push(Occurrence::new(
            &constant.ident.span(),
            format!(
                "module-level `pub const {}` pollutes the namespace; attach it to the type that \
                 uses it as an associated const",
                constant.ident
            ),
        ));
    }
    found
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::occurrences;

    fn count(source: &str) -> usize {
        occurrences(&syn::parse_file(source).unwrap()).len()
    }

    #[test]
    fn pub_module_const_is_reported() {
        assert_eq!(count("pub const MAX_CONNECTIONS: usize = 100;"), 1);
    }

    #[test]
    fn nested_module_pub_const_is_reported() {
        assert_eq!(count("mod limits { pub const MAX: usize = 100; }"), 1);
    }

    #[test]
    fn private_module_const_passes() {
        assert_eq!(count("const BUFFER_SIZE: usize = 8192;"), 0);
    }

    #[test]
    fn pub_crate_const_passes() {
        assert_eq!(count("pub(crate) const BUFFER_SIZE: usize = 8192;"), 0);
    }

    #[test]
    fn associated_const_passes() {
        assert_eq!(
            count("struct B; impl B { pub const CAPACITY: usize = 8192; }"),
            0
        );
    }
}
