// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Everything call resolution knows about the workspace beyond the
//! behavioral facts: module and crate qualifiers, trait implementors,
//! extension traits with their supertraits, and per-source build
//! targets.

use std::collections::{BTreeMap, BTreeSet};

use super::target::Target;
use crate::{semantic::expr::impl_type_name, source::Source};

/// Everything call resolution knows about the workspace beyond the
/// facts themselves.
#[derive(Debug, Default)]
pub struct ResolutionContext {
    /// Module and crate qualifiers, from [`resolution_segments`].
    pub segments:    BTreeSet<String>,
    /// Trait name to the sorted types implementing it, for dynamic
    /// dispatch: a call through `dyn Trait` produces an edge to every
    /// implementor's method.
    pub trait_impls: BTreeMap<String, Vec<String>>,
    /// Default-bodied method name to the extension traits declaring it,
    /// each with its first supertrait — the `trait Ext: Base` pattern
    /// whose blanket impl grants `Ext` to every `Base` implementor.
    pub ext_traits:  BTreeMap<String, Vec<(String, String)>>,
    /// Trait name to its first supertrait, so dispatch on an extension
    /// trait's receiver falls through to the supertrait's implementors.
    pub supertraits: BTreeMap<String, String>,
    /// Cargo build target of every source, by source index. An edge into
    /// a target the caller does not compile with is impossible, so such
    /// candidates are dropped before uniqueness is judged. An empty
    /// vector disables the check, keeping hand-built contexts permissive.
    pub targets:     Vec<Target>
}

/// Builds the full resolution context over `sources`.
#[must_use]
pub fn resolution_context(sources: &[Source]) -> ResolutionContext {
    let mut trait_impls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ext_traits: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut supertraits: BTreeMap<String, String> = BTreeMap::new();
    for source in sources {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Impl(block) => {
                    let (Some((path, _)), Some(implementor)) =
                        (&block.trait_, impl_type_name(block))
                    else {
                        continue;
                    };
                    let Some(trait_name) = path.segments.last().map(|s| s.ident.to_string())
                    else {
                        continue;
                    };
                    let implementors = trait_impls.entry(trait_name).or_default();
                    if !implementors.contains(&implementor) {
                        implementors.push(implementor);
                    }
                }
                syn::Item::Trait(definition) => {
                    let Some(base) = first_supertrait(definition) else {
                        continue;
                    };
                    let ext = definition.ident.to_string();
                    supertraits
                        .entry(ext.clone())
                        .or_insert_with(|| base.clone());
                    for trait_item in &definition.items {
                        if let syn::TraitItem::Fn(method) = trait_item
                            && method.default.is_some()
                        {
                            let entry = (ext.clone(), base.clone());
                            let entries =
                                ext_traits.entry(method.sig.ident.to_string()).or_default();
                            if !entries.contains(&entry) {
                                entries.push(entry);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for implementors in trait_impls.values_mut() {
        implementors.sort_unstable();
    }
    for entries in ext_traits.values_mut() {
        entries.sort_unstable();
    }
    ResolutionContext {
        segments: resolution_segments(sources),
        trait_impls,
        ext_traits,
        supertraits,
        targets: sources
            .iter()
            .map(|source| Target::of_path(&source.path().to_string_lossy()))
            .collect()
    }
}

/// Last segment of a trait definition's first supertrait bound.
fn first_supertrait(definition: &syn::ItemTrait) -> Option<String> {
    definition.supertraits.iter().find_map(|bound| {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            trait_bound
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
        } else {
            None
        }
    })
}

/// Builds the qualifier set for call resolution over `sources`.
///
/// The set holds the last segment of every module path plus the name of
/// every workspace crate, derived from the `<crate>/src/` path
/// convention with `-` normalized to `_`. A candidate qualified by any
/// of these resolves like a module qualifier — to a unique bare free
/// function.
#[must_use]
pub fn resolution_segments(sources: &[Source]) -> BTreeSet<String> {
    let mut segments: BTreeSet<String> = crate::semantic::module_overview::of(
        &crate::semantic::module_graph::ModuleGraph::from_sources(sources),
        sources
    )
    .modules()
    .iter()
    .filter_map(|module| module.path.rsplit("::").next())
    .map(ToOwned::to_owned)
    .collect();
    for source in sources {
        let mut components = source.path().components().peekable();
        let mut previous: Option<String> = None;
        while let Some(component) = components.next() {
            let name = component.as_os_str().to_string_lossy();
            if name == "src"
                && components.peek().is_some()
                && let Some(krate) = previous.take()
            {
                segments.insert(krate.replace('-', "_"));
                break;
            }
            previous = Some(name.into_owned());
        }
    }
    segments
}
