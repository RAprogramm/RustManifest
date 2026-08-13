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
    /// Path of each analyzed source, by its index — what a fact's source
    /// number stands for.
    pub files:       Vec<std::path::PathBuf>,
    /// What each call site reaches and what each declared body is called,
    /// both by the full path — the identity a written name does not
    /// carry. Empty where no resolver ran.
    pub identity:    Identity,
    /// Module and crate qualifiers, from [`resolution_segments_with`].
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
    pub targets:     Vec<Target>,
    /// Enum name to its written variant names. A candidate naming one of
    /// them builds a value; nothing is being called there.
    pub variants:    BTreeMap<String, BTreeSet<String>>,
    /// Whether each source is compiled by no crate root of the slice —
    /// a fixture or a loose file. Nothing in a target can call into one.
    /// Empty for a hand-built context, which then prunes nothing.
    pub loose:       Vec<bool>,
    /// Type name to the traits it carries without writing a body: the
    /// derives written above it, and the traits it implements. A member
    /// one of them grants exists on the type even though no source in
    /// the slice holds its body.
    pub granted:     BTreeMap<String, BTreeSet<String>>
}

/// Builds the full resolution context over `sources`, with no settled
/// module tree.
#[must_use]
pub fn resolution_context(sources: &[Source]) -> ResolutionContext {
    resolution_context_with(sources, None)
}

/// Builds the full resolution context over `sources`, reading which files
/// a build reaches from the module tree a resolver settled.
#[must_use]
pub fn resolution_context_with(
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>
) -> ResolutionContext {
    let identity = Identity::of(settled);
    let mut trait_impls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ext_traits: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut supertraits: BTreeMap<String, String> = BTreeMap::new();
    let mut variants: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut granted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
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
                    granted
                        .entry(implementor.clone())
                        .or_default()
                        .insert(trait_name.clone());
                    let implementors = trait_impls.entry(trait_name).or_default();
                    if !implementors.contains(&implementor) {
                        implementors.push(implementor);
                    }
                }
                syn::Item::Struct(definition) => {
                    granted
                        .entry(definition.ident.to_string())
                        .or_default()
                        .extend(derived_traits(&definition.attrs));
                }
                syn::Item::Enum(definition) => {
                    granted
                        .entry(definition.ident.to_string())
                        .or_default()
                        .extend(derived_traits(&definition.attrs));
                    variants.insert(
                        definition.ident.to_string(),
                        definition
                            .variants
                            .iter()
                            .map(|variant| variant.ident.to_string())
                            .collect()
                    );
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
        identity,
        files: named_files(sources),
        segments: resolution_segments_with(settled),
        trait_impls,
        ext_traits,
        supertraits,
        targets: sources
            .iter()
            .map(|source| Target::of_path(&source.path().to_string_lossy()))
            .collect(),
        loose: loose_sources(sources, settled),
        variants,
        granted
    }
}

/// Which sources no layout crate root reaches: a fixture beside a rule,
/// a loose script, anything cargo never compiles as part of a target.
/// A call cannot bind into such a file from a compiled one — there is
/// no build that holds both.
fn loose_sources(
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<bool> {
    let graph = crate::semantic::module_graph::ModuleGraph::of(sources, settled);
    let mut compiled = vec![false; sources.len()];
    for module in graph.modules() {
        if let Some(flag) = compiled.get_mut(module.source) {
            *flag = true;
        }
    }
    compiled.into_iter().map(|held| !held).collect()
}

/// The trait names a `#[derive(...)]` attribute writes.
fn derived_traits(attrs: &[syn::Attribute]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        drop(attr.parse_nested_meta(|meta| {
            if let Some(name) = meta.path.segments.last() {
                found.insert(name.ident.to_string());
            }
            Ok(())
        }));
    }
    found
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

/// The path of every analyzed source, by its index.
fn named_files(sources: &[Source]) -> Vec<std::path::PathBuf> {
    sources
        .iter()
        .map(|source| source.path().to_path_buf())
        .collect()
}

/// What a resolver settled about identity: the body each call reaches and
/// the name each declared body carries, both keyed by the place they are
/// written at.
///
/// A call graph built from written names has to decide which `new` a
/// candidate means; with these two tables it does not decide, it reads.
#[derive(Debug, Default)]
pub struct Identity {
    /// Full path each call site reaches, by file and byte range.
    reaches:  std::collections::BTreeMap<(std::path::PathBuf, u32, u32), String>,
    /// Full path each declared body carries, by file and byte range.
    declared: std::collections::BTreeMap<(std::path::PathBuf, u32, u32), String>
}

impl Identity {
    /// Reads both tables out of a resolution, or answers with neither.
    fn of(settled: Option<&rustmanifest_schema::Resolution>) -> Self {
        let Some(resolution) = settled else {
            return Self::default();
        };
        Self {
            reaches:  resolution
                .calls
                .iter()
                .filter_map(|call| {
                    let reaches = call.reaches.clone()?;
                    Some((keyed(&call.at), reaches))
                })
                .collect(),
            declared: resolution
                .functions
                .iter()
                .map(|declared| (keyed(&declared.at), declared.names.clone()))
                .collect()
        }
    }

    /// The full path the call written at this place reaches.
    #[must_use]
    pub fn reached(
        &self,
        file: &std::path::Path,
        site: rustmanifest_schema::Range
    ) -> Option<&str> {
        self.reaches
            .get(&(file.to_path_buf(), site.start, site.end))
            .map(String::as_str)
    }

    /// The full path the body declared at this place carries.
    #[must_use]
    pub fn declared_as(
        &self,
        file: &std::path::Path,
        at: rustmanifest_schema::Range
    ) -> Option<&str> {
        self.declared
            .get(&(file.to_path_buf(), at.start, at.end))
            .map(String::as_str)
    }

    /// Whether a resolver settled any identity at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declared.is_empty()
    }
}

/// The lookup key one settled location stands for.
fn keyed(at: &rustmanifest_schema::Location) -> (std::path::PathBuf, u32, u32) {
    (at.file.clone(), at.range.start, at.range.end)
}

/// Every name a written path may qualify a call with: each crate the
/// resolver names and each module inside it.
///
/// Read straight off the settled tree. The reading that stood here built a
/// whole module overview to take the last segment of each path, then
/// scanned every source path for a directory sitting above `src` to guess
/// the crate names — both are stated outright by the tree.
#[must_use]
pub fn resolution_segments_with(
    settled: Option<&rustmanifest_schema::Resolution>
) -> BTreeSet<String> {
    let Some(resolution) = settled else {
        return BTreeSet::new();
    };
    resolution
        .modules
        .iter()
        .flat_map(|module| module.names.split("::"))
        .map(|segment| segment.replace('-', "_"))
        .collect()
}
