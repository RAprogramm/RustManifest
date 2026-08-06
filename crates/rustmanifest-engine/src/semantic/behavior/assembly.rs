// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fact assembly: builds the workspace typing tables once, then walks
//! every source's flattened items and turns each one into the facts it
//! carries — a free function, the methods of an impl block, the `From`
//! conversions an error derive generates, and a trait's default bodies.

use super::{
    conversion::error_conversions,
    declaration::{ItemContext, is_attributed, item_flags},
    extraction::{Known, function_facts},
    imports::use_imports,
    rendering::name_rendered_texts,
    signature::generics_bounds,
    tables::TypeTables,
    trait_default::trait_default_facts,
    types::FunctionFacts
};
use crate::{semantic::expr::impl_type_name, source::Source};

/// Extracts behavioral facts for every function in `sources`; files that do
/// not parse as Rust contribute no facts.
#[must_use]
pub fn behavior(sources: &[Source]) -> Vec<FunctionFacts> {
    behavior_with(sources, None)
}

/// Extracts the facts of every function `sources` declares, with what a
/// resolver settled about them where one ran.
///
/// The overlay answers what the written text cannot — the type a call
/// hands back — and is read as plain data keyed by position, so nothing
/// here depends on where the answers came from. Without one, every
/// reading is exactly what it was.
#[must_use]
pub fn behavior_with(
    sources: &[Source],
    resolution: Option<&rustmanifest_schema::Resolution>
) -> Vec<FunctionFacts> {
    let tables = TypeTables::of(sources);
    let mut facts = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        let imports = use_imports(&items);
        let settled = settled_types(resolution, source);
        let known = Known {
            tables:  &tables,
            imports: &imports,
            settled: &settled
        };
        for item in &items {
            match item {
                syn::Item::Fn(function) => {
                    facts.push(function_facts(
                        index,
                        function.sig.ident.to_string(),
                        &ItemContext {
                            impl_type: None,
                            flags:     item_flags(
                                false,
                                &function.attrs,
                                matches!(function.vis, syn::Visibility::Public(_)),
                                false
                            ),
                            bounds:    std::collections::BTreeMap::new()
                        },
                        &function.sig,
                        &function.block,
                        &known
                    ));
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    let trait_impl = block.trait_.is_some();
                    let impl_attributed = is_attributed(&block.attrs);
                    let impl_bounds = generics_bounds(&block.generics);
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        facts.push(function_facts(
                            index,
                            format!("{type_name}::{}", method.sig.ident),
                            &ItemContext {
                                impl_type: Some(&type_name),
                                flags:     item_flags(
                                    trait_impl,
                                    &method.attrs,
                                    matches!(method.vis, syn::Visibility::Public(_)),
                                    impl_attributed
                                ),
                                bounds:    impl_bounds.clone()
                            },
                            &method.sig,
                            &method.block,
                            &known
                        ));
                    }
                }
                syn::Item::Enum(_) | syn::Item::Struct(_) => {
                    facts.extend(error_conversions(item, index));
                }
                syn::Item::Trait(definition) => {
                    facts.extend(trait_default_facts(definition, index, &known));
                }
                _ => {}
            }
        }
    }
    name_rendered_texts(&mut facts);
    facts
}

/// What a resolver settled about one source: the type each call hands
/// back, and the type of the receiver it was called on, both by the byte
/// range of the call.
#[derive(Debug, Default)]
pub(super) struct Settled {
    /// Type each call at a position hands back.
    pub(super) produced:  std::collections::BTreeMap<(u32, u32), String>,
    /// Type of the receiver each call at a position was called on.
    pub(super) receivers: std::collections::BTreeMap<(u32, u32), String>,
    /// Type holding the field read at a position, and the type that
    /// field itself has.
    pub(super) fields:    std::collections::BTreeMap<(u32, u32), (String, String)>
}

/// The name the tables speak: the outermost type of a written type, with
/// its arguments, its path, and the borrow it is held through dropped.
///
/// The resolver answers in full — `Option<&<T as Deref>::Target>` — while
/// every table here is keyed by a last segment. Reading the full spelling
/// into those tables would look up a name nothing declares.
fn outermost(written: &str) -> Option<String> {
    let head = written.split('<').next()?.trim();
    let held = head
        .trim_start_matches('&')
        .trim()
        .trim_start_matches("mut ")
        .trim();
    let named = held.rsplit("::").next()?.trim();
    (!named.is_empty()
        && named
            .chars()
            .all(|letter| letter.is_alphanumeric() || letter == '_'))
    .then(|| named.to_owned())
}

/// What a resolver settled about one source, by byte range.
///
/// Keyed by position because that is the only thing the overlay and the
/// written text certainly share: the overlay names files by path, and a
/// record naming another file is not about this one.
fn settled_types(
    resolution: Option<&rustmanifest_schema::Resolution>,
    source: &Source
) -> Settled {
    let mut settled = Settled::default();
    let Some(resolution) = resolution else {
        return settled;
    };
    for call in &resolution.calls {
        if call.at.file != source.path() {
            continue;
        }
        let at = (call.at.range.start, call.at.range.end);
        if let Some(named) = outermost(&call.returns) {
            settled.produced.insert(at, named);
        }
        if let Some(named) = call.receiver.as_deref().and_then(outermost) {
            settled.receivers.insert(at, named);
        }
    }
    for field in &resolution.fields {
        if field.at.file != source.path() {
            continue;
        }
        let (Some(owner), Some(produced)) = (outermost(&field.owner), outermost(&field.produced))
        else {
            continue;
        };
        settled.fields.insert(
            (field.at.range.start, field.at.range.end),
            (owner, produced)
        );
    }
    settled
}
