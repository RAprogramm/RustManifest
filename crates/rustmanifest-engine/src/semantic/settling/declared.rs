// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a compiler settles about the types a fixture declares: the type
//! each field holds, and the abilities the library grants those types.
//!
//! Every reading above answers from the compiler and from nothing else,
//! so a fixture asking about a store, a shape or an installed impl has
//! to state the compiler's side first. For a fixture of a few lines the
//! written type *is* the settled one — no alias, no renaming import — so
//! this states it once for the whole file.
//!
//! The abilities are the library's own, stated for the container types a
//! fixture writes. A type the table does not name is left ungranted:
//! absence is what the shape means by "not answered", here as anywhere.

use rustmanifest_schema::{
    Location, Range, Resolution, ResolvedFieldDecl, ResolvedGrant, ResolvedPath, ResolvedTypeDecl
};

use crate::source::Source;

/// The abilities the library grants the types a fixture holds values in,
/// by the name each one is written with.
const GRANTED: [(&str, &[&str]); 14] = [
    ("BTreeMap", &["Default", "Extend"]),
    ("BTreeSet", &["Default", "Extend"]),
    ("BinaryHeap", &["Default", "Extend"]),
    ("HashMap", &["Default", "Extend"]),
    ("HashSet", &["Default", "Extend"]),
    ("LinkedList", &["Default", "Extend"]),
    ("String", &["Default", "Display", "Extend"]),
    ("Vec", &["Default", "Extend"]),
    ("VecDeque", &["Default", "Extend"]),
    ("bool", &["Default", "Display"]),
    ("char", &["Default", "Display"]),
    ("u8", &["Default", "Display"]),
    ("u32", &["Default", "Display"]),
    ("usize", &["Default", "Display"])
];

/// Adds to `resolution` what a compiler settles about the types
/// `sources` declare, and the abilities the library grants them.
pub fn settle_declared(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let Some(parsed) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&parsed.items, &mut items);
        for item in items {
            match item {
                syn::Item::Struct(declared) => {
                    let fields = named_fields(&declared.fields);
                    let named = declared.ident.to_string();
                    record_derives(resolution, source, &named, &declared.attrs);
                    record(resolution, source, &named, fields);
                }
                syn::Item::Enum(declared) => {
                    let mut fields = Vec::new();
                    for variant in &declared.variants {
                        let held = variant.ident.to_string();
                        match &variant.fields {
                            syn::Fields::Unnamed(payload) => {
                                for carried in &payload.unnamed {
                                    if let Some(named) = written_type(&carried.ty) {
                                        fields.push((held.clone(), named));
                                    }
                                }
                            }
                            _ => {
                                for (name, named) in named_fields(&variant.fields) {
                                    fields.push((format!("{held}.{name}"), named));
                                }
                            }
                        }
                    }
                    let named = declared.ident.to_string();
                    record_derives(resolution, source, &named, &declared.attrs);
                    record(resolution, source, &named, fields);
                }
                _ => {}
            }
        }
    }
    resolution.settle();
}

/// Adds to `resolution` what every mention of a declared type names.
pub fn settle_mentions_of_declared(resolution: &mut Resolution, sources: &[Source]) {
    settle_mentions(resolution, sources);
    resolution.settle();
}

/// Records what every mention of a declared type names: a reading
/// asking which types a body touches answers from the item each written
/// name reaches, and a fixture writes bare names.
fn settle_mentions(resolution: &mut Resolution, sources: &[Source]) {
    let declared: Vec<(String, String)> = resolution
        .declared
        .iter()
        .map(|held| (held.named.clone(), kind_of(sources, &held.named)))
        .collect();
    for source in sources {
        let text = source.text();
        for (named, kind) in &declared {
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(named.as_str())) {
                let start = from + at;
                let end = start + named.len();
                from = end;
                let joined = text[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|letter| letter.is_alphanumeric() || letter == '_')
                    || text[end..]
                        .chars()
                        .next()
                        .is_some_and(|letter| letter.is_alphanumeric() || letter == '_');
                if joined {
                    continue;
                }
                resolution.paths.push(ResolvedPath {
                    at:        Location {
                        file:  source.path().to_path_buf(),
                        range: Range {
                            start: u32::try_from(start).unwrap_or(u32::MAX),
                            end:   u32::try_from(end).unwrap_or(u32::MAX)
                        }
                    },
                    names:     format!("crate::{named}"),
                    kind:      kind.clone(),
                    declaring: "crate".to_owned()
                });
            }
        }
    }
}

/// Which kind of item a declared name is, as the sources write it.
fn kind_of(sources: &[Source], named: &str) -> String {
    for source in sources {
        let Some(parsed) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&parsed.items, &mut items);
        for item in items {
            match item {
                syn::Item::Struct(held) if held.ident == named => return "struct".to_owned(),
                syn::Item::Enum(held) if held.ident == named => return "enum".to_owned(),
                _ => {}
            }
        }
    }
    "struct".to_owned()
}

/// What a compiler settles about what `sources` declare, and nothing
/// else.
#[must_use]
pub fn declared_over(sources: &[Source]) -> Resolution {
    let mut resolution = Resolution {
        resolver: "test".to_owned(),
        ..Resolution::default()
    };
    settle_declared(&mut resolution, sources);
    resolution
}

/// Records every trait a written derive grants the type: the compiler
/// reads a derive and a written impl as one answer, and a fixture
/// states it the same way.
fn record_derives(
    resolution: &mut Resolution,
    source: &Source,
    named: &str,
    attributes: &[syn::Attribute]
) {
    for attribute in attributes {
        if !attribute.path().is_ident("derive") {
            continue;
        }
        drop(attribute.parse_nested_meta(|held| {
            if let Some(segment) = held.path.segments.last() {
                resolution.grants.push(ResolvedGrant {
                    at:         Location {
                        file:  source.path().to_path_buf(),
                        range: Range {
                            start: 0, end: 0
                        }
                    },
                    named:      named.to_owned(),
                    trait_name: segment.ident.to_string()
                });
            }
            Ok(())
        }));
    }
}

/// Records one declared type, its fields, and the abilities the library
/// grants each field's own type.
fn record(
    resolution: &mut Resolution,
    source: &Source,
    named: &str,
    fields: Vec<(String, String)>
) {
    let at = Location {
        file:  source.path().to_path_buf(),
        range: Range {
            start: 0, end: 0
        }
    };
    for (_, held) in &fields {
        for inside in nested(held) {
            let Some(outermost) = crate::semantic::behavior::outermost_name(&inside) else {
                continue;
            };
            for (known, carried) in GRANTED {
                if known != outermost {
                    continue;
                }
                for trait_name in carried {
                    resolution.grants.push(ResolvedGrant {
                        at:         at.clone(),
                        named:      inside.clone(),
                        trait_name: (*trait_name).to_owned()
                    });
                }
            }
        }
    }
    resolution.declared.push(ResolvedTypeDecl {
        at,
        named: named.to_owned(),
        fields: fields
            .into_iter()
            .map(|(name, named)| ResolvedFieldDecl {
                name,
                named
            })
            .collect()
    });
}

/// Every type standing inside a rendering, the rendering itself first,
/// so a container held behind a handle is asked about as well.
fn nested(rendered: &str) -> Vec<String> {
    let mut held = vec![rendered.trim().to_owned()];
    let Some(inside) = rendered
        .trim()
        .split_once('<')
        .and_then(|(_, rest)| rest.strip_suffix('>'))
    else {
        return held;
    };
    let mut depth = 0usize;
    let mut start = 0usize;
    for (at, letter) in inside.char_indices() {
        match letter {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                held.extend(nested(&inside[start..at]));
                start = at + 1;
            }
            _ => {}
        }
    }
    held.extend(nested(&inside[start..]));
    held
}

/// The plainly named fields of a declaration, with the type each is
/// written as.
fn named_fields(fields: &syn::Fields) -> Vec<(String, String)> {
    let syn::Fields::Named(held) = fields else {
        return Vec::new();
    };
    held.named
        .iter()
        .filter_map(|field| {
            Some((
                field.ident.as_ref()?.to_string(),
                written_type(&field.ty)?
            ))
        })
        .collect()
}

/// A written type as one rendering, arguments kept, so a container
/// answers what it holds.
pub(super) fn written_type(written: &syn::Type) -> Option<String> {
    let rendered = quote::ToTokens::to_token_stream(written).to_string();
    Some(lifetimeless(&rendered).replace(' ', ""))
}

/// A rendering with the lifetimes a source wrote dropped: the compiler
/// renders a borrow without naming the region it holds for, and a name
/// keyed by the written one matches nothing.
fn lifetimeless(rendered: &str) -> String {
    let mut held = String::with_capacity(rendered.len());
    let mut letters = rendered.chars().peekable();
    while let Some(letter) = letters.next() {
        if letter != '\'' {
            held.push(letter);
            continue;
        }
        while letters
            .peek()
            .is_some_and(|next| next.is_alphanumeric() || *next == '_')
        {
            letters.next();
        }
    }
    held
}
