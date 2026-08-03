// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Behavioral facts: what each function *does*, extracted from its body.
//!
//! For every free function and impl method the extractor records which
//! fields it reads and writes, which workspace types it constructs, which
//! error types it can return, and which side-effect categories its body
//! touches. Facts are primitive and total — every observation becomes one
//! entry with no interpretation at extraction time — so invariant checks
//! can be derived over them later.
//!
//! Extraction is syntactic and deliberately conservative, with documented
//! limits: field owners resolve through typed receivers — `self`, typed
//! parameters, and `let` bindings whose type is written or evident from a
//! struct literal or `Type::new`/`Type::default` call — so an
//! unresolvable owner is recorded as `?`; interior mutability is
//! recognized through a fixed list of mutating methods on field
//! expressions; macro bodies other than known console macros are opaque;
//! `unsafe` and FFI mark the function rather than being traced through.

mod collector;
mod output;
mod signature;
mod types;

use rustmanifest_schema::Range;
use syn::visit::Visit;

use self::{
    collector::FactCollector,
    signature::{
        element_type, error_types, param_facts, parameter_elements, parameter_types,
        type_last_segment, wrapped_type
    }
};
pub use self::{
    output::{
        Grade, Guard, Hole, HoleSource, Multiplicity, Newline, OUTPUT_SINK_SET, OpaqueReason,
        OutputWrite, Payload, Segment, SiteGuard, Stream
    },
    types::{Effect, FunctionFacts, FunctionFlags, ParamFacts, ParamMode}
};
use super::expr::impl_type_name;
use crate::source::Source;

/// Extracts behavioral facts for every function in `sources`; files that do
/// not parse as Rust contribute no facts.
#[must_use]
pub fn behavior(sources: &[Source]) -> Vec<FunctionFacts> {
    let (fields, field_elems, field_wraps) = field_tables(sources);
    let tables = TypeTables {
        returns: return_types(sources),
        fields,
        field_elems,
        field_wraps,
        crates: crate_ids(sources)
    };
    let mut facts = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        let imports = use_imports(&items);
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
                        &tables,
                        &imports
                    ));
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    let trait_impl = block.trait_.is_some();
                    let impl_attributed = is_attributed(&block.attrs);
                    let impl_bounds = signature::generics_bounds(&block.generics);
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
                            &tables,
                            &imports
                        ));
                    }
                }
                syn::Item::Enum(_) | syn::Item::Struct(_) => {
                    facts.extend(error_conversions(item, index));
                }
                syn::Item::Trait(definition) => {
                    facts.extend(trait_default_facts(definition, index, &tables, &imports));
                }
                _ => {}
            }
        }
    }
    facts
}

/// Maps every name a file imports to its full `use` path, across all
/// modules of the file — file-level granularity is a documented
/// approximation. Glob imports stay out; a name imported twice with
/// different targets is dropped so expansion never guesses.
fn use_imports(items: &[&syn::Item]) -> std::collections::BTreeMap<String, String> {
    let mut table: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    for item in items {
        if let syn::Item::Use(declaration) = item {
            collect_use_tree(&declaration.tree, &mut Vec::new(), &mut table);
        }
    }
    settled(table)
}

/// Walks one `use` tree, recording every imported leaf under its bound
/// name with the full path it resolves to.
fn collect_use_tree(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    table: &mut std::collections::BTreeMap<String, Option<String>>
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree(&path.tree, prefix, table);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let leaf = name.ident.to_string();
            let full = if leaf == "self" {
                prefix.join("::")
            } else {
                let mut segments = prefix.clone();
                segments.push(leaf.clone());
                segments.join("::")
            };
            let key = if leaf == "self" {
                prefix.last().cloned().unwrap_or(leaf)
            } else {
                leaf
            };
            note_unique(table, key, full);
        }
        syn::UseTree::Rename(renamed) => {
            let mut segments = prefix.clone();
            segments.push(renamed.ident.to_string());
            note_unique(table, renamed.rename.to_string(), segments.join("::"));
        }
        syn::UseTree::Group(group) => {
            for entry in &group.items {
                collect_use_tree(entry, prefix, table);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

/// Returns whether a `#[derive(...)]` list names the `Error` derive —
/// the thiserror pattern whose `#[from]` fields generate `From` impls.
fn derives_error(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut named = false;
        let parsed = attr.parse_nested_meta(|meta| {
            if meta
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "Error")
            {
                named = true;
            }
            Ok(())
        });
        parsed.is_ok() && named
    })
}

/// Returns whether a field carries the `#[from]` attribute.
fn from_attributed(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("from"))
}

/// Facts of a trait definition's default-bodied methods, qualified by
/// the trait's own name.
fn trait_default_facts(
    definition: &syn::ItemTrait,
    index: usize,
    tables: &TypeTables,
    imports: &std::collections::BTreeMap<String, String>
) -> Vec<FunctionFacts> {
    let trait_name = definition.ident.to_string();
    let public = matches!(definition.vis, syn::Visibility::Public(_));
    let trait_bounds = signature::generics_bounds(&definition.generics);
    let mut facts = Vec::new();
    for trait_item in &definition.items {
        let syn::TraitItem::Fn(method) = trait_item else {
            continue;
        };
        let Some(block) = &method.default else {
            continue;
        };
        facts.push(function_facts(
            index,
            format!("{trait_name}::{}", method.sig.ident),
            &ItemContext {
                impl_type: Some(&trait_name),
                flags:     item_flags(true, &method.attrs, public, false),
                bounds:    trait_bounds.clone()
            },
            &method.sig,
            block,
            tables,
            imports
        ));
    }
    facts
}

/// Synthesized `From` conversion facts of one item: the `Error` derive
/// guarantees `impl From<Source> for Owner` for every `#[from]` field.
fn error_conversions(item: &syn::Item, index: usize) -> Vec<FunctionFacts> {
    let mut facts = Vec::new();
    match item {
        syn::Item::Enum(definition) if derives_error(&definition.attrs) => {
            let owner = definition.ident.to_string();
            for variant in &definition.variants {
                for field in &variant.fields {
                    if from_attributed(&field.attrs)
                        && let Some(source_type) = type_last_segment(&field.ty)
                    {
                        facts.push(conversion_fact(index, &owner, source_type));
                    }
                }
            }
        }
        syn::Item::Struct(definition) if derives_error(&definition.attrs) => {
            let owner = definition.ident.to_string();
            for field in &definition.fields {
                if from_attributed(&field.attrs)
                    && let Some(source_type) = type_last_segment(&field.ty)
                {
                    facts.push(conversion_fact(index, &owner, source_type));
                }
            }
        }
        _ => {}
    }
    facts
}

/// Synthesizes the fact of a derive-generated `From` conversion: the
/// `Error` derive guarantees `impl From<Source> for Owner` for every
/// `#[from]` field, so the generated function exists with exactly this
/// shape even though no syntax carries its body.
fn conversion_fact(source: usize, owner: &str, source_type: String) -> FunctionFacts {
    FunctionFacts {
        function: format!("{owner}::from"),
        source,
        range: Range {
            start: 0, end: 0
        },
        reads: Vec::new(),
        writes: Vec::new(),
        builds: Vec::new(),
        constructs: vec![owner.to_owned()],
        error_types: Vec::new(),
        effects: Vec::new(),
        params: vec![ParamFacts {
            name:        "source".to_owned(),
            type_name:   source_type,
            mode:        ParamMode::Owned,
            uses:        1,
            field_reads: 0
        }],
        flags: FunctionFlags::from_bits(FunctionFlags::TRAIT_IMPL | FunctionFlags::PUBLIC),
        calls: Vec::new(),
        clones: 0,
        allocations: 0,
        output_writes: Vec::new()
    }
}

/// Returns whether any attribute's last path segment is `test`, covering
/// `#[test]` and `#[tokio::test]`-style harness attributes.
fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
    })
}

/// Declaration context of one item: where it sits and what it declares.
struct ItemContext<'name> {
    /// Impl self type for methods, `None` for free functions.
    impl_type: Option<&'name str>,
    /// Declaration flags of the item.
    flags:     FunctionFlags,
    /// Impl-block generic parameters to their first trait bound, so a
    /// generic-typed field resolves as its bound inside the block's
    /// methods.
    bounds:    std::collections::BTreeMap<String, String>
}

/// Workspace typing tables shared by every function's fact extraction.
struct TypeTables {
    /// Function names to their return types, scoped by source.
    returns:     Scoped<Returned>,
    /// `Type.field` keys to field type names, scoped by source.
    fields:      Scoped<String>,
    /// `Type.field` keys of collection fields to element types.
    field_elems: Scoped<String>,
    /// `Type.field` keys of single-generic wrapper fields to the
    /// wrapped inner type.
    field_wraps: Scoped<String>,
    /// Crate identity of every source, for crate-proximity resolution.
    crates:      Vec<String>
}

/// Records `value` under `key`, dropping the key on conflicting
/// observations so typing through the table never guesses.
fn note_unique(
    table: &mut std::collections::BTreeMap<String, Option<String>>,
    key: String,
    value: String
) {
    table
        .entry(key)
        .and_modify(|known| {
            if known.as_deref() != Some(value.as_str()) {
                *known = None;
            }
        })
        .or_insert(Some(value));
}

/// Keeps only the keys that settled on exactly one observation.
fn settled(
    table: std::collections::BTreeMap<String, Option<String>>
) -> std::collections::BTreeMap<String, String> {
    table
        .into_iter()
        .filter_map(|(key, value)| value.map(|v| (key, v)))
        .collect()
}

/// Typing table keeping every observation with its source file, so a
/// colliding short key resolves by the consumer's proximity — the same
/// file first, then the same crate — instead of being dropped. A
/// collision proximity cannot settle stays unresolved, never guessed.
#[derive(Debug, Clone)]
pub(super) struct Scoped<V> {
    /// Observations per short key, each with its source index.
    entries: std::collections::BTreeMap<String, Vec<(usize, V)>>
}

impl<V> Default for Scoped<V> {
    fn default() -> Self {
        Self {
            entries: std::collections::BTreeMap::new()
        }
    }
}

impl<V: Clone + PartialEq> Scoped<V> {
    /// Records one observation of `key` from `source`.
    fn note(&mut self, key: String, source: usize, value: V) {
        let observations = self.entries.entry(key).or_default();
        if !observations
            .iter()
            .any(|(seen, known)| *seen == source && *known == value)
        {
            observations.push((source, value));
        }
    }

    /// Resolves `key` for a consumer in `source`: a unique observation
    /// wins outright; among colliding ones, a single distinct value
    /// from the consumer's file wins, then a single distinct value
    /// from the consumer's crate.
    pub(super) fn resolve(&self, key: &str, source: usize, crates: &[String]) -> Option<&V> {
        let observations = self.entries.get(key)?;
        if let [(_, only)] = observations.as_slice() {
            return Some(only);
        }
        single_value(observations.iter().filter(|(origin, _)| *origin == source)).or_else(|| {
            let own = crates.get(source)?;
            single_value(
                observations
                    .iter()
                    .filter(|(origin, _)| crates.get(*origin).is_some_and(|theirs| theirs == own))
            )
        })
    }
}

/// The single distinct value among the observations, if exactly one.
fn single_value<'observations, V: PartialEq>(
    observations: impl Iterator<Item = &'observations (usize, V)>
) -> Option<&'observations V> {
    let mut found: Option<&V> = None;
    for (_, value) in observations {
        match found {
            None => found = Some(value),
            Some(seen) if seen == value => {}
            Some(_) => return None
        }
    }
    found
}

/// Crate identity of every source, from the path component preceding
/// its `src` directory.
fn crate_ids(sources: &[Source]) -> Vec<String> {
    sources
        .iter()
        .map(|source| {
            let path = source.path().to_string_lossy().replace('\\', "/");
            let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
            components
                .iter()
                .position(|component| *component == "src")
                .and_then(|position| position.checked_sub(1))
                .and_then(|index| components.get(index))
                .map(|name| (*name).to_owned())
                .unwrap_or_default()
        })
        .collect()
}

/// Maps every named field of a uniquely named struct — keyed
/// `Type.field` — to its type's last segment, every collection-typed
/// field to its element type, and every single-generic wrapper field
/// to its wrapped inner type.
fn field_tables(sources: &[Source]) -> (Scoped<String>, Scoped<String>, Scoped<String>) {
    let mut plain = Scoped::default();
    let mut elems = Scoped::default();
    let mut wraps = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            let syn::Item::Struct(definition) = item else {
                continue;
            };
            for (position, field) in definition.fields.iter().enumerate() {
                let member = field
                    .ident
                    .as_ref()
                    .map_or_else(|| position.to_string(), ToString::to_string);
                let key = format!("{}.{member}", definition.ident);
                if let Some(type_name) = type_last_segment(&field.ty) {
                    plain.note(key.clone(), index, type_name);
                }
                if let Some(element) = element_type(&field.ty) {
                    elems.note(key.clone(), index, element);
                }
                if let Some(inner) = wrapped_type(&field.ty) {
                    wraps.note(key, index, inner);
                }
            }
        }
    }
    (plain, elems, wraps)
}

/// Return type of one workspace function as the typing table sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Returned {
    /// Last segment of the written return type, `Self` resolved.
    pub(super) plain:   String,
    /// Success type inside a `Result`/`Option` return, `Self` resolved.
    pub(super) success: Option<String>,
    /// Error type inside a `Result` return, `Self` resolved.
    pub(super) error:   Option<String>
}

/// Maps every uniquely named free function — and every associated
/// function keyed `Type::name` — to its return type; keys with
/// conflicting returns map to nothing, so typing through this table
/// never guesses.
fn return_types(sources: &[Source]) -> Scoped<Returned> {
    let mut table = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    record_return(
                        &mut table,
                        function.sig.ident.to_string(),
                        index,
                        None,
                        &function.sig
                    );
                }
                syn::Item::Impl(block) => {
                    let Some(owner) = impl_type_name(block) else {
                        continue;
                    };
                    for impl_item in &block.items {
                        if let syn::ImplItem::Fn(method) = impl_item {
                            record_return(
                                &mut table,
                                format!("{owner}::{}", method.sig.ident),
                                index,
                                Some(&owner),
                                &method.sig
                            );
                        }
                    }
                }
                syn::Item::Trait(definition) => {
                    let owner = definition.ident.to_string();
                    for trait_item in &definition.items {
                        if let syn::TraitItem::Fn(method) = trait_item
                            && method.default.is_some()
                        {
                            record_return(
                                &mut table,
                                format!("{owner}::{}", method.sig.ident),
                                index,
                                Some(&owner),
                                &method.sig
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
    table
}

/// Records one function's return under `key` with its source.
fn record_return(
    table: &mut Scoped<Returned>,
    key: String,
    source: usize,
    owner: Option<&str>,
    signature: &syn::Signature
) {
    let Some(returned) = returned_of(signature, owner) else {
        return;
    };
    table.note(key, source, returned);
}

/// Reads a signature's plain return type and, for `Result`/`Option`
/// returns, the success type carried in the first type argument.
fn returned_of(signature: &syn::Signature, owner: Option<&str>) -> Option<Returned> {
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return None;
    };
    let syn::Type::Path(path) = ty.as_ref() else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let plain = resolve_self(segment.ident.to_string(), owner);
    let success = if segment.ident == "Result" || segment.ident == "Option" {
        type_argument(segment, 0).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    let error = if segment.ident == "Result" {
        type_argument(segment, 1).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    Some(Returned {
        plain,
        success,
        error
    })
}

/// Last segment of the `position`-th angle-bracketed type argument.
fn type_argument(segment: &syn::PathSegment, position: usize) -> Option<String> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| {
            if let syn::GenericArgument::Type(syn::Type::Path(path)) = argument {
                path.path.segments.last().map(|s| s.ident.to_string())
            } else {
                None
            }
        })
        .nth(position)
}

/// Substitutes `Self` with the impl owner when one is known.
fn resolve_self(name: String, owner: Option<&str>) -> String {
    if name == "Self" {
        owner.map_or(name, str::to_owned)
    } else {
        name
    }
}

/// Packs declaration flags from the trait context, attributes, and
/// visibility.
fn item_flags(
    trait_impl: bool,
    attrs: &[syn::Attribute],
    public: bool,
    inherited_attr: bool
) -> FunctionFlags {
    let mut bits = 0u8;
    if trait_impl {
        bits |= FunctionFlags::TRAIT_IMPL;
    }
    if is_test(attrs) {
        bits |= FunctionFlags::TEST;
    }
    if public {
        bits |= FunctionFlags::PUBLIC;
    }
    if inherited_attr || is_attributed(attrs) {
        bits |= FunctionFlags::ATTRIBUTED;
    }
    FunctionFlags::from_bits(bits)
}

/// Returns whether any attribute is neither a doc comment nor a `test`
/// harness marker.
fn is_attributed(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let last = attr
            .path()
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        !matches!(last.as_deref(), Some("doc" | "test"))
    })
}

/// Extracts the facts of one function body against its signature.
fn function_facts(
    source: usize,
    function: String,
    context: &ItemContext<'_>,
    signature: &syn::Signature,
    block: &syn::Block,
    tables: &TypeTables,
    imports: &std::collections::BTreeMap<String, String>
) -> FunctionFacts {
    let bytes = signature.ident.span().byte_range();
    let params = param_facts(signature, context.impl_type);
    let mut collector = FactCollector {
        facts:       FunctionFacts {
            function,
            source,
            range: Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            },
            reads: Vec::new(),
            writes: Vec::new(),
            builds: Vec::new(),
            constructs: Vec::new(),
            error_types: Vec::new(),
            effects: Vec::new(),
            params,
            flags: context.flags,
            calls: Vec::new(),
            clones: 0,
            allocations: 0,
            output_writes: Vec::new()
        },
        param_types: parameter_types(signature, context.impl_type),
        elem_types:  parameter_elements(signature),
        fn_returns:  tables.returns.clone(),
        field_types: tables.fields.clone(),
        field_elems: tables.field_elems.clone(),
        field_wraps: tables.field_wraps.clone(),
        crates:      tables.crates.clone(),
        src:         source,
        bounds:      context.bounds.clone(),
        local_wraps: std::collections::BTreeMap::new(),
        imports:     imports.clone(),
        fresh:       std::collections::BTreeSet::new(),
        guards:      Vec::new(),
        prior_exits: 0,
        dead:        false
    };
    if matches!(signature.safety, syn::Safety::Unsafe(_)) {
        collector.facts.effects.push(Effect::Unsafe);
    }
    collector.facts.error_types = error_types(signature);
    collector.visit_block(block);
    collector.facts.reads.sort();
    collector.facts.reads.dedup();
    collector.facts.writes.sort();
    collector.facts.writes.dedup();
    collector.facts.constructs.sort();
    collector.facts.constructs.dedup();
    collector.facts.effects.sort();
    collector.facts.effects.dedup();
    collector.facts.calls.sort();
    collector.facts.calls.dedup();
    collector.facts
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Effect, FunctionFacts, behavior};
    use crate::source::Source;

    fn facts(text: &str) -> Vec<FunctionFacts> {
        behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
    }

    fn named<'a>(all: &'a [FunctionFacts], name: &str) -> &'a FunctionFacts {
        all.iter().find(|facts| facts.function == name).unwrap()
    }

    #[test]
    fn self_field_writes_and_reads_are_qualified() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet {\n    fn credit(&mut self, amount: \
             i64) { self.balance += amount; }\n    fn balance(&self) -> i64 { self.balance }\n}"
        );
        let credit = named(&all, "Wallet::credit");
        assert_eq!(credit.writes, vec!["Wallet.balance"]);
        assert_eq!(credit.reads, vec!["Wallet.balance"]);
        let getter = named(&all, "Wallet::balance");
        assert_eq!(getter.reads, vec!["Wallet.balance"]);
        assert!(getter.writes.is_empty());
    }

    #[test]
    fn typed_parameter_receivers_resolve_the_owner() {
        let all = facts(
            "struct Wallet { balance: i64 }\nfn drain(wallet: &mut Wallet) { wallet.balance = 0; \
             }\nfn peek(unknown: impl Fn()) { }"
        );
        assert_eq!(named(&all, "drain").writes, vec!["Wallet.balance"]);
    }

    #[test]
    fn unresolvable_owner_is_marked_honestly() {
        let all =
            facts("fn poke(tuple: (u8, u8)) { tuple.0; }\nfn chain() { helper().field = 1; }");
        assert_eq!(named(&all, "chain").writes, vec!["?.field"]);
    }

    #[test]
    fn mutating_methods_on_fields_count_as_writes() {
        let all = facts(
            "struct State { cache: Vec<u8> }\nimpl State { fn add(&self, byte: u8) { \
             self.cache.push(byte); } }"
        );
        assert_eq!(named(&all, "State::add").writes, vec!["State.cache"]);
    }

    #[test]
    fn struct_literals_and_self_literals_record_constructs() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn new() -> Self { Self { balance: 0 } \
             } }\nfn make() -> Wallet { Wallet { balance: 1 } }"
        );
        assert_eq!(named(&all, "make").constructs, vec!["Wallet"]);
        assert_eq!(named(&all, "Wallet::new").constructs, vec!["Self"]);
    }

    #[test]
    fn error_types_come_from_result_signatures() {
        let all = facts(
            "struct ParseError;\nfn parse(input: &str) -> Result<u32, ParseError> { \
             Ok(input.len() as u32) }\nfn plain() -> u32 { 0 }"
        );
        assert_eq!(named(&all, "parse").error_types, vec!["ParseError"]);
        assert!(named(&all, "plain").error_types.is_empty());
    }

    #[test]
    fn effects_cover_unsafe_fs_net_process_env_console() {
        let all = facts(
            "fn risky() { unsafe { core::ptr::null::<u8>(); } }\nfn save() { \
             std::fs::write(\"x\", \"y\").ok(); }\nfn fetch() { \
             std::net::TcpStream::connect(\"h\").ok(); }\nfn run() { \
             std::process::Command::new(\"ls\"); }\nfn home() { std::env::var(\"HOME\").ok(); \
             }\nfn shout() { println!(\"hi\"); }"
        );
        assert_eq!(named(&all, "risky").effects, vec![Effect::Unsafe]);
        assert_eq!(named(&all, "save").effects, vec![Effect::FileSystem]);
        assert_eq!(named(&all, "fetch").effects, vec![Effect::Network]);
        assert_eq!(named(&all, "run").effects, vec![Effect::Process]);
        assert_eq!(named(&all, "home").effects, vec![Effect::Environment]);
        assert_eq!(named(&all, "shout").effects, vec![Effect::ConsoleIo]);
    }

    #[test]
    fn trait_default_methods_carry_their_owner() {
        let all = facts(
            "pub trait Reporter {\n    fn emit(&self);\n    fn shout(&self) { \
             println!(\"hi\"); }\n}"
        );
        let shout = named(&all, "Reporter::shout");
        assert_eq!(shout.effects, vec![Effect::ConsoleIo]);
        assert!(all.iter().all(|f| f.function != "Reporter::emit"));
    }

    #[test]
    fn imported_names_classify_through_their_use_paths() {
        let all = facts(
            "use std::fs::remove_dir_all;\nuse std::fs::write as write_file;\nuse \
             std::{env, fs::{self, File}};\nfn wipe() { remove_dir_all(\"x\").ok(); }\nfn \
             dump() { write_file(\"x\", \"y\").ok(); }\nfn open() { File::open(\"x\").ok(); \
             }\nfn module() { fs::read(\"x\").ok(); }\nfn who() { env::var(\"USER\").ok(); }"
        );
        assert_eq!(named(&all, "wipe").effects, vec![Effect::FileSystem]);
        assert_eq!(named(&all, "dump").effects, vec![Effect::FileSystem]);
        assert_eq!(named(&all, "open").effects, vec![Effect::FileSystem]);
        assert_eq!(named(&all, "module").effects, vec![Effect::FileSystem]);
        assert_eq!(named(&all, "who").effects, vec![Effect::Environment]);
    }

    #[test]
    fn impl_bounds_type_generic_fields_and_awaits_reach_poll() {
        let all = facts(
            "struct WriteAll<W> { writer: W }\nimpl<W: AsyncWrite> Future for WriteAll<W> {\n    \
             fn poll(&mut self) -> u8 { self.writer.poll_write(); 0 }\n}\nstruct Done;\nimpl \
             Done { fn write_all(&self) -> WriteAll<Done> { WriteAll { writer: Done } } }\nasync \
             fn drive(done: Done) { done.write_all().await; }"
        );
        let poll = named(&all, "WriteAll::poll");
        assert!(poll.calls.contains(&"AsyncWrite::poll_write".to_owned()));
        let drive = named(&all, "drive");
        assert!(drive.calls.contains(&"WriteAll::poll".to_owned()));
    }

    #[test]
    fn try_sites_record_the_implicit_conversion() {
        let all = facts(
            "struct AppError;\nstruct DbError;\nfn load() -> Result<u8, DbError> { Ok(0) }\nfn \
             handle() -> Result<u8, AppError> { let v = load()?; Ok(v) }\nfn same() -> \
             Result<u8, DbError> { let v = load()?; Ok(v) }"
        );
        assert!(
            named(&all, "handle")
                .calls
                .contains(&"AppError::from".to_owned())
        );
        assert!(
            !named(&all, "same")
                .calls
                .contains(&"DbError::from".to_owned())
        );
    }

    #[test]
    fn error_derives_synthesize_their_from_conversions() {
        let all = facts(
            "#[derive(Debug, Error)]\nenum AppError {\n    #[error(\"db\")]\n    Db(#[from] \
             DbError)\n}\nstruct DbError;\n#[derive(Clone)]\nenum Plain { A(#[from] u8) }"
        );
        let from = named(&all, "AppError::from");
        assert_eq!(from.constructs, vec!["AppError"]);
        assert_eq!(from.params[0].type_name, "DbError");
        assert!(all.iter().all(|f| f.function != "Plain::from"));
    }

    #[test]
    fn tuple_fields_type_their_receivers() {
        let all = facts(
            "struct Wire(TcpStream);\nimpl Wire { fn pump(&self) { self.0.read(&mut []).ok(); } \
             }\nstruct Layer(Wrapper<TcpStream>);\nimpl Layer { fn go(&self) { \
             self.0.do_io(|s| { s.read(&mut []).ok(); }); } }"
        );
        assert_eq!(named(&all, "Wire::pump").effects, vec![Effect::Network]);
        assert_eq!(named(&all, "Layer::go").effects, vec![Effect::Network]);
    }

    #[test]
    fn tuple_field_reads_stay_out_of_the_fact_channel() {
        let all = facts("fn poke(pair: (u8, u8)) -> u8 { pair.0 }");
        assert!(named(&all, "poke").reads.is_empty());
    }

    #[test]
    fn colliding_type_names_resolve_by_consumer_proximity() {
        let sources = vec![
            Source::new(
                PathBuf::from("app/src/net.rs"),
                "pub struct Client { inner: TcpStream }\nimpl Client { pub fn go(&self) { \
                 self.inner.read(&mut []).ok(); } }"
                    .to_owned()
            ),
            Source::new(
                PathBuf::from("app/src/disk.rs"),
                "pub struct Client { inner: File }\nimpl Client { pub fn go(&self) { \
                 self.inner.read(&mut []).ok(); } }"
                    .to_owned()
            ),
        ];
        let all = behavior(&sources);
        let net = all
            .iter()
            .find(|f| f.function == "Client::go" && f.source == 0)
            .unwrap();
        let disk = all
            .iter()
            .find(|f| f.function == "Client::go" && f.source == 1)
            .unwrap();
        assert_eq!(net.effects, vec![Effect::Network]);
        assert_eq!(disk.effects, vec![Effect::FileSystem]);
    }

    #[test]
    fn service_calls_dispatch_to_the_wrapped_service() {
        let all = facts(
            "struct Layered<S> { inner: S }\nstruct HyperService;\nimpl HyperService { fn \
             call(&self) { std::net::TcpStream::connect(\"h\").ok(); } }\nstruct Client { \
             hyper: Layered<HyperService> }\nimpl Client { fn execute(&self) { let svc = \
             self.hyper.clone(); svc.call(); } }"
        );
        assert!(
            named(&all, "Client::execute")
                .calls
                .contains(&"HyperService::call".to_owned())
        );
    }

    #[test]
    fn nested_field_chains_type_recursively() {
        let all = facts(
            "struct App { db: Db }\nstruct Db { conn: TcpStream }\nimpl App { fn ping(&self) { \
             self.db.conn.read(&mut []).ok(); } }"
        );
        assert_eq!(named(&all, "App::ping").effects, vec![Effect::Network]);
    }

    #[test]
    fn fieldless_projections_type_as_their_receiver() {
        let all = facts(
            "struct Sender { sock: TcpStream }\nstruct Projection;\nimpl Sender {\n    fn \
             project(&mut self) -> Projection { Projection }\n    fn pump(&mut self) { let me = \
             self.project(); me.sock.read(&mut []).ok(); }\n}\nstruct Real { count: u32 }\nimpl \
             Real { fn project(&self) -> Real { Real { count: 0 } } }"
        );
        assert_eq!(named(&all, "Sender::pump").effects, vec![Effect::Network]);
    }

    #[test]
    fn pinned_receivers_type_through_the_inner_expression() {
        let all = facts(
            "fn pump(stream: &mut TcpStream) { Pin::new(&mut *stream).read(&mut []).ok(); }"
        );
        assert_eq!(named(&all, "pump").effects, vec![Effect::Network]);
    }

    #[test]
    fn wrapped_closure_parameters_type_through_the_field_inner() {
        let all = facts(
            "struct Src { inner: IoSource<TcpStream> }\nimpl Src { fn pump(&self) { \
             self.inner.do_io(|stream| { stream.read(&mut []).ok(); }); } }\nstruct Two { \
             pair: Both<TcpStream, File> }\nimpl Two { fn poke(&self) { \
             self.pair.do_io(|value| { value.read(&mut []).ok(); }); } }"
        );
        assert_eq!(named(&all, "Src::pump").effects, vec![Effect::Network]);
        assert!(named(&all, "Two::poke").effects.is_empty());
    }

    #[test]
    fn std_method_axioms_fire_on_typed_receivers() {
        let all = facts(
            "struct Log { sink: File }\nimpl Log { fn flush(&self) { \
             self.sink.sync_all().ok(); } }\nfn pump(stream: &mut TcpStream) { \
             stream.read(&mut []).ok(); }\nfn untyped(stream: &mut Unknown) { \
             stream.read(&mut []).ok(); }"
        );
        assert_eq!(named(&all, "Log::flush").effects, vec![Effect::FileSystem]);
        assert_eq!(named(&all, "pump").effects, vec![Effect::Network]);
        assert!(named(&all, "untyped").effects.is_empty());
    }

    #[test]
    fn conflicting_imports_expand_to_nothing() {
        let all = facts(
            "mod a { use std::fs::remove_dir_all; }\nmod b { use crate::cleanup::remove_dir_all; \
             }\nfn wipe() { remove_dir_all(\"x\").ok(); }"
        );
        assert!(named(&all, "wipe").effects.is_empty());
    }

    #[test]
    fn workspace_imports_stay_silent() {
        let all = facts(
            "use crate::storage::remove_dir_all;\nfn wipe() { remove_dir_all(\"x\").ok(); }"
        );
        assert!(named(&all, "wipe").effects.is_empty());
    }

    #[test]
    fn unsafe_fn_signature_is_an_unsafe_effect() {
        let all = facts("unsafe fn raw() {}");
        assert_eq!(named(&all, "raw").effects, vec![Effect::Unsafe]);
    }

    #[test]
    fn let_bindings_resolve_owners_by_annotation_literal_and_constructor() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn new() -> Self { Self { balance: 0 \
             } } }\nfn annotated() { let wallet: Wallet = make(); wallet.balance = 1; }\nfn \
             literal() { let wallet = Wallet { balance: 0 }; wallet.balance = 2; }\nfn \
             constructed() { let wallet = Wallet::new(); wallet.balance = 3; }\nfn opaque() { \
             let wallet = make(); wallet.balance = 4; }"
        );
        assert_eq!(named(&all, "annotated").writes, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "literal").builds, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "constructed").builds, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "opaque").writes, vec!["?.balance"]);
    }

    #[test]
    fn associated_factories_type_their_bindings() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn open() -> Self { Self { balance: 0 \
             } } }\nfn opened() { let wallet = Wallet::open(); wallet.balance = 1; }"
        );
        assert_eq!(named(&all, "opened").builds, vec!["Wallet.balance"]);
    }

    #[test]
    fn try_unwrapped_factories_yield_the_success_type() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn open() -> Result<Self, String> { \
             Ok(Self { balance: 0 }) } }\nfn opened() -> Result<(), String> { let wallet = \
             Wallet::open()?; wallet.balance = 1; Ok(()) }"
        );
        assert_eq!(named(&all, "opened").builds, vec!["Wallet.balance"]);
    }

    #[test]
    fn awaited_factories_resolve_through_the_table() {
        let all = facts(
            "struct Pool { size: u32 }\nimpl Pool { async fn connect() -> Result<Self, String> { \
             Ok(Self { size: 0 }) } }\nasync fn ready() -> Result<(), String> { let pool = \
             Pool::connect().await?; pool.size = 1; Ok(()) }"
        );
        assert_eq!(named(&all, "ready").builds, vec!["Pool.size"]);
    }

    #[test]
    fn assembling_a_fresh_value_is_a_build_not_a_write() {
        let all = facts(
            "struct Order { items: u32 }\nimpl Order { fn load() -> Result<Self, String> { \
             Ok(Self { items: 0 }) } }\nstruct Repo;\nimpl Repo { fn get_order(&self) -> \
             Result<Order, String> { let mut order = Order::load()?; order.items = 3; Ok(order) \
             } }"
        );
        let getter = named(&all, "Repo::get_order");
        assert!(getter.writes.is_empty());
        assert_eq!(getter.builds, vec!["Order.items"]);
    }

    #[test]
    fn rebinding_a_fresh_name_to_state_restores_write_facts() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn new() -> Self { Self { balance: 0 \
             } } }\nfn rebind(source: &mut Wallet) { let wallet = Wallet::new(); let wallet = \
             source; wallet.balance = 1; }"
        );
        let facts = named(&all, "rebind");
        assert!(facts.builds.is_empty());
        assert_eq!(facts.writes, vec!["Wallet.balance"]);
    }

    #[test]
    fn knowledge_packs_type_chains_and_carry_effects() {
        let all = facts(
            "async fn fetch(client: &Client) -> Result<String, String> { let body = \
             client.get(\"u\").send().await.unwrap().text().await.unwrap(); Ok(body) }\nfn \
             probe() { let dir = tempfile::tempdir().unwrap(); dir.close().unwrap(); }"
        );
        assert!(named(&all, "fetch").effects.contains(&Effect::Network));
        let probe = named(&all, "probe");
        assert!(probe.effects.contains(&Effect::FileSystem));
        assert!(probe.calls.contains(&"TempDir::close".to_owned()));
    }

    #[test]
    fn sql_macros_split_into_read_and_write_effects() {
        let all = facts(
            "fn load() { let row = query_as!(Row, \"SELECT id FROM users\"); }\nfn save() { \
             query!(\"INSERT INTO users (id) VALUES ($1)\", 1); }\nfn opaque() { \
             query!(dynamic_sql); }"
        );
        assert_eq!(named(&all, "load").effects, vec![Effect::DatabaseRead]);
        assert_eq!(named(&all, "save").effects, vec![Effect::DatabaseWrite]);
        assert!(named(&all, "opaque").effects.is_empty());
    }

    #[test]
    fn conflicting_associated_returns_stay_untyped() {
        let all = facts(
            "mod a { pub struct Cfg { pub set: u32 } impl Cfg { pub fn load() -> Self { Self { \
             set: 0 } } } }\nmod b { pub struct Cfg; impl Cfg { pub fn load() -> u32 { 0 } } \
             }\nfn use_it() { let cfg = a::Cfg::load(); cfg.set = 1; }"
        );
        assert_eq!(named(&all, "use_it").writes, vec!["?.set"]);
    }

    #[test]
    fn param_modes_and_use_shapes_are_profiled() {
        let all = facts(
            "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { report.total \
             }\nfn consume(report: Report) -> Report { report }\nfn update(report: &mut Report) \
             { report.total = 0; }"
        );
        let summarize = &named(&all, "summarize").params[0];
        assert_eq!(summarize.mode, super::ParamMode::Owned);
        assert_eq!((summarize.uses, summarize.field_reads), (1, 1));
        let consume = &named(&all, "consume").params[0];
        assert_eq!((consume.uses, consume.field_reads), (1, 0));
        let update = &named(&all, "update").params[0];
        assert_eq!(update.mode, super::ParamMode::BorrowedMut);
        assert_eq!((update.uses, update.field_reads), (1, 0));
    }

    #[test]
    fn self_receiver_modes_are_profiled() {
        let all = facts(
            "struct W;\nimpl W {\n    fn by_ref(&self) {}\n    fn by_mut(&mut self) {}\n    fn \
             by_value(self) {}\n}"
        );
        assert_eq!(
            named(&all, "W::by_ref").params[0].mode,
            super::ParamMode::Borrowed
        );
        assert_eq!(
            named(&all, "W::by_mut").params[0].mode,
            super::ParamMode::BorrowedMut
        );
        assert_eq!(
            named(&all, "W::by_value").params[0].mode,
            super::ParamMode::Owned
        );
    }

    #[test]
    fn clones_and_allocations_are_counted() {
        let all = facts(
            "fn busy(input: &str) -> String {\n    let copy = input.to_owned();\n    let boxed = \
             Box::new(1u8);\n    let list = vec![1, 2];\n    let text = format!(\"x\", copy, \
             list, boxed);\n    text.clone()\n}"
        );
        let busy = named(&all, "busy");
        assert_eq!(busy.clones, 2);
        assert_eq!(busy.allocations, 3);
    }

    #[test]
    fn extraction_is_deterministic() {
        let text = "struct W { b: i64 }\nimpl W { fn f(&mut self) { self.b += 1; println!(\"x\"); \
                    } }";
        assert_eq!(facts(text), facts(text));
    }
}
