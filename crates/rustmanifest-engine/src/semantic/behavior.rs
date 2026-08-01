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
    signature::{error_types, param_facts, parameter_types}
};
use super::expr::impl_type_name;
use crate::source::Source;

/// Extracts behavioral facts for every function in `sources`; files that do
/// not parse as Rust contribute no facts.
#[must_use]
pub fn behavior(sources: &[Source]) -> Vec<FunctionFacts> {
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
                        &function.block
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
                            &method.block
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
    block: &syn::Block
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
            constructs: Vec::new(),
            error_types: Vec::new(),
            effects: Vec::new(),
            params,
            flags: context.flags,
            calls: Vec::new(),
            clones: 0,
            allocations: 0
        },
        param_types: parameter_types(signature, context.impl_type)
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
        assert_eq!(named(&all, "literal").writes, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "constructed").writes, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "opaque").writes, vec!["?.balance"]);
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
