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
                            flags: item_flags(
                                false,
                                &function.attrs,
                                matches!(function.vis, syn::Visibility::Public(_)),
                                false
                            ),
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
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        facts.push(function_facts(
                            index,
                            format!("{type_name}::{}", method.sig.ident),
                            &ItemContext {
                                flags: item_flags(
                                    trait_impl,
                                    &method.attrs,
                                    matches!(method.vis, syn::Visibility::Public(_)),
                                    impl_attributed
                                )
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
    pub(super) fields:    std::collections::BTreeMap<(u32, u32), (String, String)>,
    /// Type of the expression written at a position, whatever its shape.
    ///
    /// This one is not about calls or fields: it answers for every
    /// expression the resolver settled, which is what lets a reading stop
    /// deciding a type from how the expression was spelled.
    pub(super) types:     std::collections::BTreeMap<(u32, u32), String>,
    /// Parameters of the function whose name is written at a position, as
    /// the compiler read its signature.
    pub(super) params:    std::collections::BTreeMap<(u32, u32), Vec<super::types::ParamFacts>>,
    /// Type the function whose name is written at a position hands back,
    /// as the compiler renders it in full.
    pub(super) returns:   std::collections::BTreeMap<(u32, u32), String>,
    /// Full path of the body the call at a position reaches.
    ///
    /// The written name says which method was spelled; this says which
    /// one it is. Two types with a `default` share the spelling and
    /// never the path.
    pub(super) reached:   std::collections::BTreeMap<(u32, u32), String>,
    /// Type of the expression at a position, as the compiler renders it
    /// in full.
    ///
    /// The outermost name answers most questions; the halves of a
    /// fallible answer are written inside the rendering and nowhere else.
    pub(super) rendered:  std::collections::BTreeMap<(u32, u32), String>,
    /// How the call at a position takes the value it is written on.
    pub(super) takes:     std::collections::BTreeMap<(u32, u32), super::types::ParamMode>,
    /// Full path the written path at a position reaches.
    pub(super) named:     std::collections::BTreeMap<(u32, u32), String>,
    /// Kind of item the path written at a position reaches.
    ///
    /// A path is either a name this body bound or an item declared
    /// somewhere; the reading below can only guess which by looking the
    /// last segment up in what the workspace happens to declare. The
    /// resolver knows, and a path it settles to no item is a local — which
    /// is why absence here is an answer and not a gap.
    pub(super) items:     std::collections::BTreeMap<(u32, u32), String>
}

/// How a declared position is taken, in this tier's own vocabulary.
const fn declared_mode(mode: rustmanifest_schema::ResolvedMode) -> super::types::ParamMode {
    match mode {
        rustmanifest_schema::ResolvedMode::Owned => super::types::ParamMode::Owned,
        rustmanifest_schema::ResolvedMode::Borrowed => super::types::ParamMode::Borrowed,
        rustmanifest_schema::ResolvedMode::BorrowedMut => super::types::ParamMode::BorrowedMut
    }
}

/// The name the tables speak: the outermost type of a written type, with
/// its arguments, its path, the borrow it is held through and the
/// dynamism it is reached through dropped.
///
/// The resolver answers in full — `Option<&<T as Deref>::Target>` — while
/// every table here is keyed by a last segment. Reading the full spelling
/// into those tables would look up a name nothing declares. A receiver
/// reached through a trait object renders as `&dyn Sink`, whose outermost
/// name is the trait: keeping the keyword would leave every call written
/// on such a value nameless.
pub(in crate::semantic) fn outermost(written: &str) -> Option<String> {
    let borrowless = written
        .trim()
        .trim_start_matches('&')
        .trim()
        .trim_start_matches("mut ")
        .trim()
        .trim_start_matches("dyn ")
        .trim();
    if let Some(named) = unnamed_shape(borrowless) {
        return Some(named.to_owned());
    }
    let head = borrowless.split('<').next()?.trim();
    let without_borrow = head
        .trim_start_matches('&')
        .trim()
        .trim_start_matches("mut ")
        .trim();
    let named = without_borrow.rsplit("::").next()?.trim();
    (!named.is_empty()
        && named
            .chars()
            .all(|letter| letter.is_alphanumeric() || letter == '_'))
    .then(|| named.to_owned())
}

/// The name the tables speak for a shape the language spells with
/// punctuation rather than with a path.
///
/// A resolver renders a slice as `&[String]` and a tuple as `(u8, u8)`.
/// Neither has a last path segment to take, so a reading that only knows
/// how to take one leaves every call written on such a value untyped —
/// which is most calls on a borrowed sequence.
fn unnamed_shape(borrowless: &str) -> Option<&'static str> {
    if borrowless.starts_with('(') {
        return Some("tuple");
    }
    if !borrowless.starts_with('[') {
        return None;
    }
    Some(if borrowless.contains(';') {
        "array"
    } else {
        "slice"
    })
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
        if let Some(mode) = call.takes {
            settled.takes.insert(at, declared_mode(mode));
        }
        if let Some(reached) = call.reaches.clone() {
            settled.reached.insert(at, reached);
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
    for typed in &resolution.types {
        if typed.at.file != source.path() {
            continue;
        }
        if let Some(named) = outermost(&typed.rendered) {
            settled
                .types
                .insert((typed.at.range.start, typed.at.range.end), named);
        }
        settled.rendered.insert(
            (typed.at.range.start, typed.at.range.end),
            typed.rendered.clone()
        );
    }
    for held in &resolution.signatures {
        if held.at.file != source.path() {
            continue;
        }
        settled.returns.insert(
            (held.at.range.start, held.at.range.end),
            held.returns.clone()
        );
        settled.params.insert(
            (held.at.range.start, held.at.range.end),
            held.params
                .iter()
                .filter_map(|param| {
                    Some(super::types::ParamFacts {
                        name:        param.name.clone(),
                        type_name:   outermost(&param.named)?,
                        mode:        match param.mode {
                            rustmanifest_schema::ResolvedMode::Owned => {
                                super::types::ParamMode::Owned
                            }
                            rustmanifest_schema::ResolvedMode::Borrowed => {
                                super::types::ParamMode::Borrowed
                            }
                            rustmanifest_schema::ResolvedMode::BorrowedMut => {
                                super::types::ParamMode::BorrowedMut
                            }
                        },
                        uses:        0,
                        field_reads: 0
                    })
                })
                .collect()
        );
    }
    for path in &resolution.paths {
        if path.at.file != source.path() {
            continue;
        }
        settled
            .items
            .insert((path.at.range.start, path.at.range.end), path.kind.clone());
        settled
            .named
            .insert((path.at.range.start, path.at.range.end), path.names.clone());
    }
    settled
}
