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
mod signature;
mod types;

use rustmanifest_schema::Range;
use syn::visit::Visit;

pub use self::types::{Effect, FunctionFacts, FunctionFlags, ParamFacts, ParamMode};
use self::{
    collector::FactCollector,
    signature::{
        element_type, error_types, param_facts, parameter_elements, parameter_types,
        type_last_segment
    }
};
use super::expr::impl_type_name;
use crate::source::Source;

/// Extracts behavioral facts for every function in `sources`; files that do
/// not parse as Rust contribute no facts.
#[must_use]
pub fn behavior(sources: &[Source]) -> Vec<FunctionFacts> {
    let (fields, field_elems) = field_tables(sources);
    let tables = TypeTables {
        returns: return_types(sources),
        fields,
        field_elems
    };
    let mut facts = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
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
                            )
                        },
                        &function.sig,
                        &function.block,
                        &tables
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
                                impl_type: Some(&type_name),
                                flags:     item_flags(
                                    trait_impl,
                                    &method.attrs,
                                    matches!(method.vis, syn::Visibility::Public(_)),
                                    impl_attributed
                                )
                            },
                            &method.sig,
                            &method.block,
                            &tables
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    facts
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
    flags:     FunctionFlags
}

/// Workspace typing tables shared by every function's fact extraction.
struct TypeTables {
    /// Uniquely named functions to their return types.
    returns:     std::collections::BTreeMap<String, Returned>,
    /// `Type.field` keys of uniquely named structs to field type names.
    fields:      std::collections::BTreeMap<String, String>,
    /// `Type.field` keys of collection-typed fields to element types.
    field_elems: std::collections::BTreeMap<String, String>
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

/// Maps every named field of a uniquely named struct — keyed
/// `Type.field` — to its type's last segment, and every
/// collection-typed field to its element type.
fn field_tables(
    sources: &[Source]
) -> (
    std::collections::BTreeMap<String, String>,
    std::collections::BTreeMap<String, String>
) {
    let mut plain: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    let mut elems: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    for source in sources {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            let syn::Item::Struct(definition) = item else {
                continue;
            };
            let syn::Fields::Named(named) = &definition.fields else {
                continue;
            };
            for field in &named.named {
                let Some(name) = field.ident.as_ref() else {
                    continue;
                };
                let key = format!("{}.{name}", definition.ident);
                if let Some(type_name) = type_last_segment(&field.ty) {
                    note_unique(&mut plain, key.clone(), type_name);
                }
                if let Some(element) = element_type(&field.ty) {
                    note_unique(&mut elems, key, element);
                }
            }
        }
    }
    (settled(plain), settled(elems))
}

/// Return type of one workspace function as the typing table sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Returned {
    /// Last segment of the written return type, `Self` resolved.
    pub(super) plain:   String,
    /// Success type inside a `Result`/`Option` return, `Self` resolved.
    pub(super) success: Option<String>
}

/// Maps every uniquely named free function — and every associated
/// function keyed `Type::name` — to its return type; keys with
/// conflicting returns map to nothing, so typing through this table
/// never guesses.
fn return_types(sources: &[Source]) -> std::collections::BTreeMap<String, Returned> {
    let mut table: std::collections::BTreeMap<String, Option<Returned>> =
        std::collections::BTreeMap::new();
    for source in sources {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    record_return(&mut table, function.sig.ident.to_string(), None, &function.sig);
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
        .into_iter()
        .filter_map(|(name, returned)| returned.map(|r| (name, r)))
        .collect()
}

/// Records one function's return under `key`, dropping the key on
/// conflicting observations.
fn record_return(
    table: &mut std::collections::BTreeMap<String, Option<Returned>>,
    key: String,
    owner: Option<&str>,
    signature: &syn::Signature
) {
    let Some(returned) = returned_of(signature, owner) else {
        return;
    };
    table
        .entry(key)
        .and_modify(|known| {
            if known.as_ref() != Some(&returned) {
                *known = None;
            }
        })
        .or_insert(Some(returned));
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
        first_type_argument(segment).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    Some(Returned {
        plain,
        success
    })
}

/// Last segment of the first angle-bracketed type argument.
fn first_type_argument(segment: &syn::PathSegment) -> Option<String> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| {
        if let syn::GenericArgument::Type(syn::Type::Path(path)) = argument {
            path.path.segments.last().map(|s| s.ident.to_string())
        } else {
            None
        }
    })
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
    tables: &TypeTables
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
            allocations: 0
        },
        param_types: parameter_types(signature, context.impl_type),
        elem_types:  parameter_elements(signature),
        fn_returns:  tables.returns.clone(),
        field_types: tables.fields.clone(),
        field_elems: tables.field_elems.clone(),
        fresh:       std::collections::BTreeSet::new()
    };
    if signature.unsafety.is_some() {
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
