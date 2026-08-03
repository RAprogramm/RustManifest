// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The library file under derivation, and every read taken from it.
//!
//! One file is parsed once and questioned many times: which writer chain
//! a handle's own field declares, where a proving expression sits inside
//! a free function, where it sits inside a method written for a named
//! type. Each answer is a byte range in that file, and the fact built
//! around it quotes the text at that range, so nothing leaves this
//! module that the file itself does not say.

use std::path::Path;

use rustmanifest_schema::Range;
use syn::spanned::Spanned;

use super::{
    buffering::{buffering_kind, type_names},
    citation::{byte_range, quoted},
    expression::{ExpressionKind, locate},
    fact::StdFact
};
use crate::semantic::expr::impl_type_name;

/// The library file under derivation, parsed once and read from many
/// times.
pub(super) struct Evidence<'a> {
    /// Path the facts cite.
    pub(super) file:  &'a Path,
    /// Full text, the only source of a quoted snippet.
    pub(super) text:  &'a str,
    /// Every item of the file, modules flattened.
    pub(super) items: &'a [&'a syn::Item]
}

impl Evidence<'_> {
    /// Builds one fact, resolving the line and quoting the library's own
    /// text at the proving range.
    pub(super) fn fact(&self, property: &str, observed: &str, range: Range) -> StdFact {
        StdFact {
            property: property.to_owned(),
            observed: observed.to_owned(),
            file: self.file.to_path_buf(),
            range,
            line: self
                .text
                .get(..range.start as usize)
                .map_or(1, |before| before.matches('\n').count() + 1),
            snippet: quoted(self.text, range),
            doc: None
        }
    }

    /// Buffering of one stream, read from the writer chain its handle
    /// type declares. A chain naming a writer this derivation does not
    /// recognize yields nothing at all rather than a guess.
    pub(super) fn buffering(&self, handle: &str, stream: &str) -> Option<StdFact> {
        let (chain, range) = self.writer_chain(handle)?;
        let kind = buffering_kind(&chain)?;
        Some(self.fact(
            &format!("{kind}:{stream}"),
            &format!(
                "the `{handle}` handle declares the writer chain `{}`",
                chain.join(" -> ")
            ),
            range
        ))
    }

    /// Type names the handle's own field declares, outermost first, with
    /// the byte range of that declaration.
    fn writer_chain(&self, handle: &str) -> Option<(Vec<String>, Range)> {
        for item in self.items {
            let syn::Item::Struct(declared) = item else {
                continue;
            };
            if declared.ident != handle {
                continue;
            }
            for field in &declared.fields {
                let names = type_names(&field.ty);
                if names.is_empty() {
                    continue;
                }
                return Some((names, byte_range(&field.span().byte_range())));
            }
        }
        None
    }

    /// Byte range of a proving expression inside a free function's body.
    pub(super) fn free_expression(&self, function: &str, kind: ExpressionKind) -> Option<Range> {
        for item in self.items {
            let syn::Item::Fn(declared) = item else {
                continue;
            };
            if declared.sig.ident != function {
                continue;
            }
            if let Some(range) = locate(&declared.block, kind) {
                return Some(range);
            }
        }
        None
    }

    /// Byte range of a proving expression inside a method written for
    /// the named type, across every implementation block that type has:
    /// the library splits the write trait between a handle and a borrow
    /// of it, and the proving call sits in the borrow's block.
    pub(super) fn method_expression(
        &self,
        owner: &str,
        method: &str,
        kind: ExpressionKind
    ) -> Option<Range> {
        for item in self.items {
            let syn::Item::Impl(block) = item else {
                continue;
            };
            if impl_type_name(block).as_deref() != Some(owner) {
                continue;
            }
            for member in &block.items {
                let syn::ImplItem::Fn(declared) = member else {
                    continue;
                };
                if declared.sig.ident != method {
                    continue;
                }
                if let Some(range) = locate(&declared.block, kind) {
                    return Some(range);
                }
            }
        }
        None
    }
}
