// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which discipline a stream's writer chain implements.
//!
//! A handle declares its writer as a type, and the type names spell out
//! what happens to the bytes: a chain naming a buffering writer buffers
//! the way that writer does, a chain ending in a raw handle buffers
//! nothing. Only the writers listed here are recognized, and a chain
//! naming none of them yields no knowledge rather than the convenient
//! assumption that it buffers nothing.

/// Writer types this derivation recognizes as holding a buffer, with the
/// discipline each one implements.
const BUFFERING_WRITERS: [(&str, &str); 2] = [
    ("LineWriter", "line-buffered"),
    ("BufWriter", "block-buffered")
];

/// Types that hand their bytes straight to the operating system handle.
/// A chain ending in one of them, naming no buffering writer on the way,
/// buffers nothing.
const RAW_HANDLES: [&str; 2] = ["StdoutRaw", "StderrRaw"];

/// Buffering discipline a writer chain implements. A chain naming no
/// recognized writer and ending outside the raw handles yields `None`:
/// an unknown writer is reported as no knowledge, never as no buffer.
pub(super) fn buffering_kind(chain: &[String]) -> Option<&'static str> {
    if let Some((_, kind)) = BUFFERING_WRITERS
        .iter()
        .find(|(writer, _)| chain.iter().any(|name| name == writer))
    {
        return Some(kind);
    }
    let last = chain.last()?;
    RAW_HANDLES.contains(&last.as_str()).then_some("unbuffered")
}

/// Every type name appearing in a declared type, outermost first,
/// looking through references, parentheses and groups so a handle field
/// written as a borrow still yields its writer chain.
pub(super) fn type_names(declared: &syn::Type) -> Vec<String> {
    let mut names = Vec::new();
    match declared {
        syn::Type::Reference(borrowed) => return type_names(&borrowed.elem),
        syn::Type::Paren(inner) => return type_names(&inner.elem),
        syn::Type::Group(inner) => return type_names(&inner.elem),
        syn::Type::Path(path) => {
            for segment in &path.path.segments {
                names.push(segment.ident.to_string());
                if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let syn::GenericArgument::Type(inner) = argument {
                            names.extend(type_names(inner));
                        }
                    }
                }
            }
        }
        _ => ()
    }
    names
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    #[test]
    fn a_writer_chain_is_read_through_a_borrow_and_judged_by_its_own_names() {
        let declared: syn::Type =
            syn::parse_str("&'static ReentrantLock<RefCell<LineWriter<StdoutRaw>>>").unwrap();
        assert_eq!(
            super::type_names(&declared),
            ["ReentrantLock", "RefCell", "LineWriter", "StdoutRaw"]
        );
        assert_eq!(
            super::buffering_kind(&super::type_names(&declared)),
            Some("line-buffered")
        );

        let raw: syn::Type = syn::parse_str("&'static ReentrantLock<RefCell<StderrRaw>>").unwrap();
        assert_eq!(
            super::buffering_kind(&super::type_names(&raw)),
            Some("unbuffered")
        );

        let unknown: syn::Type =
            syn::parse_str("&'static ReentrantLock<SomeFutureWriter>").unwrap();
        assert_eq!(super::buffering_kind(&super::type_names(&unknown)), None);
    }
}
