// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The extraction of a type with its retinue: the declaration, every
//! `impl` block of the type in the file, and the tests that exercise
//! it, moved as one semantic unit into a file named after the type.
//!
//! The mechanics are the function extraction's; the one new concern is
//! visibility in the opposite direction. Privacy descends, so the
//! moved unit keeps seeing what stays — but the staying file stops
//! seeing the private fields and methods that left. Exactly the fields
//! and inherent methods the remaining text mentions gain `pub(super)`,
//! widened by proven need and nothing else. A private type itself
//! gains `pub(super)` and is re-exported, so every existing mention
//! keeps compiling.

use rustmanifest_schema::Range;

use super::{
    Extraction, carried, disjoint, module_file, mounted,
    names::{bound_names, mentioned_idents},
    ranged, sibling_names, stale_imports, swallowed, written_of
};
use crate::source::Source;

/// One found declaration: its name, privacy, span, and the offset of
/// the keyword a promotion writes before.
struct Declared {
    name:       String,
    private:    bool,
    span:       Range,
    keyword_at: u32
}

/// The struct or enum declared over `offset`, if any.
fn declared_at(parsed: &syn::File, offset: u32) -> Option<Declared> {
    parsed.items.iter().find_map(|item| {
        let (ident, vis, span, keyword_at) = match item {
            syn::Item::Struct(held) => (
                &held.ident,
                &held.vis,
                ranged(syn::spanned::Spanned::span(held)),
                ranged(held.struct_token.span).start
            ),
            syn::Item::Enum(held) => (
                &held.ident,
                &held.vis,
                ranged(syn::spanned::Spanned::span(held)),
                ranged(held.enum_token.span).start
            ),
            _ => return None
        };
        ((span.start..span.end).contains(&offset)).then(|| Declared {
            name: ident.to_string(),
            private: matches!(vis, syn::Visibility::Inherited),
            span,
            keyword_at
        })
    })
}

/// The `pub(super)` insertion offsets proven by the staying file: the
/// private fields and inherent methods its remaining text mentions.
fn needed_promotions(
    parsed: &syn::File,
    impls: &[&syn::ItemImpl],
    name: &str,
    remaining: &std::collections::BTreeSet<String>
) -> Vec<u32> {
    let mut promotions = Vec::new();
    if let Some(syn::Item::Struct(held)) = parsed
        .items
        .iter()
        .find(|item| matches!(item, syn::Item::Struct(held) if held.ident == name))
    {
        for field in &held.fields {
            if let Some(ident) = &field.ident
                && matches!(field.vis, syn::Visibility::Inherited)
                && remaining.contains(&ident.to_string())
            {
                promotions.push(ranged(syn::spanned::Spanned::span(field)).start);
            }
        }
    }
    for block in impls {
        if block.trait_.is_some() {
            continue;
        }
        for item in &block.items {
            if let syn::ImplItem::Fn(method) = item
                && matches!(method.vis, syn::Visibility::Inherited)
                && remaining.contains(&method.sig.ident.to_string())
            {
                promotions.push(ranged(method.sig.fn_token.span).start);
            }
        }
    }
    promotions
}

/// Plans the extraction of the struct or enum declared at `offset`,
/// with its impls and tests, or nothing where no whole type stands
/// there.
#[must_use]
pub fn extract_type(sources: &[Source], file: usize, offset: u32) -> Option<Extraction> {
    let source = sources.get(file)?;
    let text = source.text();
    let parsed = syn::parse_file(text).ok()?;
    let found = declared_at(&parsed, offset)?;
    let (name, private) = (found.name, found.private);
    let mut pieces = vec![found.span];
    let impls: Vec<&syn::ItemImpl> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(held) if implements(held, &name) => Some(held),
            _ => None
        })
        .collect();
    for block in &impls {
        pieces.push(ranged(syn::spanned::Spanned::span(*block)));
    }
    let mentions: std::collections::BTreeSet<String> = pieces
        .iter()
        .filter_map(|span| text.get(span.start as usize..span.end as usize))
        .flat_map(mentioned_idents)
        .collect();
    let remaining = remaining_mentions(text, &parsed, &pieces);
    let mut promotions = needed_promotions(&parsed, &impls, &name, &remaining);
    if private {
        promotions.push(found.keyword_at);
    }
    let mut lines = Vec::new();
    for item in &parsed.items {
        if let syn::Item::Use(used) = item {
            let bound = bound_names(&used.tree);
            let wanted = bound.is_empty() || bound.iter().any(|name| mentions.contains(name));
            if wanted && let Some(written) = written_of(text, used) {
                lines.push(written.trim_end().to_owned());
            }
        }
    }
    let siblings = sibling_names(&parsed, &name);
    let bridged: Vec<&str> = siblings
        .iter()
        .filter(|sibling| mentions.contains(sibling.as_str()))
        .map(String::as_str)
        .collect();
    if !bridged.is_empty() {
        lines.push(format!("use super::{{{}}};", bridged.join(", ")));
    }
    let mut body = Vec::new();
    for span in &pieces {
        body.push(promoted_piece(text, *span, &promotions)?);
    }
    let mut new_text = if lines.is_empty() {
        format!("{}\n", body.join("\n\n"))
    } else {
        format!("{}\n\n{}\n", lines.join("\n"), body.join("\n\n"))
    };
    let followed = carried::carried_tests(text, &parsed, &name, &siblings);
    if let Some(block) = &followed.block {
        new_text.push('\n');
        new_text.push_str(block);
        new_text.push('\n');
    }
    let snake = snake_name(&name);
    let mut edits: Vec<(Range, String)> = pieces
        .iter()
        .map(|span| (swallowed(text, *span), String::new()))
        .collect();
    edits.extend(followed.edits);
    let mod_line = format!("mod {snake};\n");
    let reexport = if private {
        format!("use self::{snake}::{name};\n")
    } else {
        format!("pub use self::{snake}::{name};\n")
    };
    edits.extend(mounted(text, &parsed, &snake, mod_line, reexport));
    for removed in stale_imports(text, &parsed, &pieces) {
        edits.push((swallowed(text, removed), String::new()));
    }
    Some(Extraction {
        new_file: module_file(source.path(), &snake)?,
        new_text,
        edits: disjoint(edits)
    })
}

/// Whether the impl block's self type is the named type.
fn implements(block: &syn::ItemImpl, name: &str) -> bool {
    if let syn::Type::Path(path) = block.self_ty.as_ref() {
        path.path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
    } else {
        false
    }
}

/// The idents the file still mentions once the pieces and the imports
/// are cut away.
fn remaining_mentions(
    text: &str,
    parsed: &syn::File,
    pieces: &[Range]
) -> std::collections::BTreeSet<String> {
    let mut cuts: Vec<Range> = pieces.to_vec();
    for item in &parsed.items {
        if let syn::Item::Use(used) = item {
            cuts.push(ranged(syn::spanned::Spanned::span(used)));
        }
    }
    cuts.sort_by_key(|range| range.start);
    let mut kept = String::new();
    let mut cursor = 0usize;
    for cut in &cuts {
        if let Some(piece) = text.get(cursor..cut.start as usize) {
            kept.push_str(piece);
        }
        cursor = cursor.max(cut.end as usize);
    }
    kept.push_str(text.get(cursor..).unwrap_or(""));
    mentioned_idents(&kept)
}

/// One moved piece with every `pub(super)` promotion inside it applied.
fn promoted_piece(text: &str, span: Range, promotions: &[u32]) -> Option<String> {
    let mut held = text.get(span.start as usize..span.end as usize)?.to_owned();
    let mut inside: Vec<u32> = promotions
        .iter()
        .copied()
        .filter(|at| (span.start..span.end).contains(at))
        .collect();
    inside.sort_unstable_by(|left, right| right.cmp(left));
    for at in inside {
        held.insert_str((at - span.start) as usize, "pub(super) ");
    }
    Some(held)
}

/// The snake-case file name of one type name.
fn snake_name(name: &str) -> String {
    let mut held = String::new();
    for (position, part) in name.chars().enumerate() {
        if part.is_uppercase() {
            if position > 0 {
                held.push('_');
            }
            held.extend(part.to_lowercase());
        } else {
            held.push(part);
        }
    }
    held
}
