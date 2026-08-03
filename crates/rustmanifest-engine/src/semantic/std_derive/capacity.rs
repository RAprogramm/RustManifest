// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The heap a stream's buffer costs, read from the library's own
//! constructor.
//!
//! The number is not written here. It is the literal the buffering
//! writer hands to its own with-capacity constructor, so a toolchain
//! that changes the size changes this fact, and a constructor that stops
//! passing a literal yields nothing rather than a stale constant.

use std::path::Path;

use rustmanifest_schema::Range;

use super::fact::StdFact;

/// Capacity the buffering writer reserves when the stream builds it,
/// read from the literal the library passes to its own constructor.
///
/// The number is the heap the first write to the stream costs, and it is
/// the library's number, not one written here: a constructor that stops
/// passing a literal yields nothing rather than a stale constant.
pub(super) fn buffer_capacity(root: &Path) -> Option<StdFact> {
    let path = root.join("std/src/io/buffered/linewriter.rs");
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed = syn::parse_file(&text).ok()?;
    let mut items = Vec::new();
    crate::ast::flattened(&parsed.items, &mut items);
    for item in items {
        let syn::Item::Impl(block) = item else {
            continue;
        };
        for member in &block.items {
            let syn::ImplItem::Fn(method) = member else {
                continue;
            };
            if method.sig.ident != "new" {
                continue;
            }
            let Some((capacity, range)) = capacity_argument(&method.block) else {
                continue;
            };
            let line = text
                .get(..range.start as usize)
                .map_or(1, |before| before.matches('\n').count() + 1);
            return Some(StdFact {
                property: "buffer-capacity".to_owned(),
                observed: capacity.to_string(),
                file: path,
                range,
                line,
                snippet: text
                    .get(range.start as usize..range.end as usize)
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
                doc: None
            });
        }
    }
    None
}

/// The capacity literal a body passes to a with-capacity constructor.
fn capacity_argument(block: &syn::Block) -> Option<(u64, Range)> {
    use syn::spanned::Spanned;

    struct Locate {
        found: Option<(u64, Range)>
    }
    impl<'ast> syn::visit::Visit<'ast> for Locate {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = call.func.as_ref()
                && path
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "with_capacity")
                && let Some(syn::Expr::Lit(literal)) = call.args.first()
                && let syn::Lit::Int(value) = &literal.lit
                && let Ok(capacity) = value.base10_parse::<u64>()
                && self.found.is_none()
            {
                let span = call.span().byte_range();
                self.found = Some((
                    capacity,
                    Range {
                        start: u32::try_from(span.start).unwrap_or(u32::MAX),
                        end:   u32::try_from(span.end).unwrap_or(u32::MAX)
                    }
                ));
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    let mut locate = Locate {
        found: None
    };
    syn::visit::Visit::visit_block(&mut locate, block);
    locate.found
}
