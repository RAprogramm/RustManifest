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

use std::{collections::BTreeMap, fmt};

use rustmanifest_schema::Range;
use syn::visit::Visit;

use crate::source::Source;

/// Side-effect category observed in a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Effect {
    /// `unsafe` block or `unsafe fn` signature.
    Unsafe,
    /// Filesystem access through `std::fs` or `File` operations.
    FileSystem,
    /// Network access through `std::net` socket types.
    Network,
    /// Child-process control through `std::process`.
    Process,
    /// Environment access through `std::env`.
    Environment,
    /// Console output through print-family macros.
    ConsoleIo
}

impl fmt::Display for Effect {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unsafe => "unsafe",
            Self::FileSystem => "fs",
            Self::Network => "net",
            Self::Process => "process",
            Self::Environment => "env",
            Self::ConsoleIo => "console"
        };
        out.write_str(name)
    }
}

/// How one parameter takes and uses its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamFacts {
    /// Parameter name as written.
    pub name:        String,
    /// Last segment of the declared type.
    pub type_name:   String,
    /// How the parameter receives the value.
    pub mode:        ParamMode,
    /// Total observed uses of the parameter in the body.
    pub uses:        u32,
    /// Uses that are only field reads on the parameter.
    pub field_reads: u32
}

/// Ownership mode of a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// Taken by value: the caller gives up ownership.
    Owned,
    /// Taken by shared reference.
    Borrowed,
    /// Taken by mutable reference.
    BorrowedMut
}

impl fmt::Display for ParamMode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Owned => "owned",
            Self::Borrowed => "borrowed",
            Self::BorrowedMut => "borrowed-mut"
        })
    }
}

/// Behavioral facts of one function or method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    /// Qualified name: `Type::method` for impl methods, bare name for free
    /// functions.
    pub function:    String,
    /// Index of the source the function was parsed from.
    pub source:      usize,
    /// Byte range of the function's identifier.
    pub range:       Range,
    /// Fields read, as `Owner.field`; `?` owner when the receiver's type
    /// is not resolvable from the signature.
    pub reads:       Vec<String>,
    /// Fields written, same notation as `reads`; covers plain and compound
    /// assignment plus known mutating methods on a field expression.
    pub writes:      Vec<String>,
    /// Types constructed through struct literals.
    pub constructs:  Vec<String>,
    /// Error types the signature can return (`Result<_, E>` → `E`).
    pub error_types: Vec<String>,
    /// Side-effect categories observed, deduplicated and sorted.
    pub effects:     Vec<Effect>,
    /// Ownership profile of each parameter, in declaration order.
    pub params:      Vec<ParamFacts>,
    /// Whether the function is a method of a trait impl, where the
    /// signature is dictated by the trait.
    pub trait_impl:  bool,
    /// `.clone()` / `.to_owned()` / `.to_string()` call sites observed.
    pub clones:      u32,
    /// Heap allocation sites observed (`Box::new`, `Vec::new`, `vec![]`,
    /// `String::from`, `format!`).
    pub allocations: u32
}

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
                        None,
                        false,
                        &function.sig,
                        &function.block
                    ));
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    let trait_impl = block.trait_.is_some();
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        facts.push(function_facts(
                            index,
                            format!("{type_name}::{}", method.sig.ident),
                            Some(&type_name),
                            trait_impl,
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

/// Renders the last path segment of an impl's self type.
fn impl_type_name(block: &syn::ItemImpl) -> Option<String> {
    let syn::Type::Path(path) = block.self_ty.as_ref() else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// Extracts the facts of one function body against its signature.
fn function_facts(
    source: usize,
    function: String,
    impl_type: Option<&str>,
    trait_impl: bool,
    signature: &syn::Signature,
    block: &syn::Block
) -> FunctionFacts {
    let bytes = signature.ident.span().byte_range();
    let params = param_facts(signature, impl_type);
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
            trait_impl,
            clones: 0,
            allocations: 0
        },
        param_types: parameter_types(signature, impl_type)
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
    collector.facts
}

/// Builds the ownership profile of every parameter, `self` included.
fn param_facts(signature: &syn::Signature, impl_type: Option<&str>) -> Vec<ParamFacts> {
    let mut params = Vec::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(receiver) => {
                let Some(impl_type) = impl_type else {
                    continue;
                };
                let mode = if receiver.reference.is_none() {
                    ParamMode::Owned
                } else if receiver.mutability.is_some() {
                    ParamMode::BorrowedMut
                } else {
                    ParamMode::Borrowed
                };
                params.push(ParamFacts {
                    name: "self".to_owned(),
                    type_name: impl_type.to_owned(),
                    mode,
                    uses: 0,
                    field_reads: 0
                });
            }
            syn::FnArg::Typed(argument) => {
                let syn::Pat::Ident(pattern) = argument.pat.as_ref() else {
                    continue;
                };
                let Some(type_name) = type_last_segment(&argument.ty) else {
                    continue;
                };
                let mode = match argument.ty.as_ref() {
                    syn::Type::Reference(reference) if reference.mutability.is_some() => {
                        ParamMode::BorrowedMut
                    }
                    syn::Type::Reference(_) => ParamMode::Borrowed,
                    _ => ParamMode::Owned
                };
                params.push(ParamFacts {
                    name: pattern.ident.to_string(),
                    type_name,
                    mode,
                    uses: 0,
                    field_reads: 0
                });
            }
        }
    }
    params
}

/// Maps parameter names to the last segment of their declared type;
/// `self` maps to the impl type.
fn parameter_types(
    signature: &syn::Signature,
    impl_type: Option<&str>
) -> BTreeMap<String, String> {
    let mut types = BTreeMap::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(_) => {
                if let Some(impl_type) = impl_type {
                    types.insert("self".to_owned(), impl_type.to_owned());
                }
            }
            syn::FnArg::Typed(argument) => {
                if let (syn::Pat::Ident(pattern), Some(type_name)) =
                    (argument.pat.as_ref(), type_last_segment(&argument.ty))
                {
                    types.insert(pattern.ident.to_string(), type_name);
                }
            }
        }
    }
    types
}

/// Renders the last path segment of a type, looking through references.
fn type_last_segment(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => type_last_segment(&reference.elem),
        _ => None
    }
}

/// Extracts error type names from a `Result<_, E>` return type.
fn error_types(signature: &syn::Signature) -> Vec<String> {
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return Vec::new();
    };
    let syn::Type::Path(path) = ty.as_ref() else {
        return Vec::new();
    };
    let Some(segment) = path.path.segments.last() else {
        return Vec::new();
    };
    if segment.ident != "Result" {
        return Vec::new();
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Vec::new();
    };
    arguments
        .args
        .iter()
        .skip(1)
        .filter_map(|argument| {
            let syn::GenericArgument::Type(ty) = argument else {
                return None;
            };
            type_last_segment(ty)
        })
        .collect()
}

/// Mutating methods that count as a write to the field they are called on.
const MUTATING_METHODS: [&str; 12] = [
    "borrow_mut",
    "clear",
    "get_mut",
    "insert",
    "lock",
    "push",
    "remove",
    "replace",
    "set",
    "store",
    "take",
    "write"
];

/// Console-output macros classified as [`Effect::ConsoleIo`].
const CONSOLE_MACROS: [&str; 5] = ["dbg", "eprint", "eprintln", "print", "println"];

/// Syntax visitor accumulating facts from one function body.
struct FactCollector {
    /// Facts collected so far.
    facts:       FunctionFacts,
    /// Parameter name to type-name mapping from the signature.
    param_types: BTreeMap<String, String>
}

impl FactCollector {
    /// Renders a field expression as `Owner.field` when the base is a
    /// typed parameter or `self`, or `?.field` otherwise.
    fn field_key(&self, field: &syn::ExprField) -> Option<String> {
        let syn::Member::Named(name) = &field.member else {
            return None;
        };
        let owner = match field.base.as_ref() {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map_or("?", String::as_str),
            _ => "?"
        };
        Some(format!("{owner}.{name}"))
    }

    /// Records the outermost field of an assignment target as a write and
    /// counts the base parameter's use so a written-through parameter never
    /// looks read-only.
    fn record_write_target(&mut self, target: &syn::Expr) {
        if let syn::Expr::Field(field) = target {
            if let Some(key) = self.field_key(field) {
                self.facts.writes.push(key);
            }
            if let syn::Expr::Path(path) = field.base.as_ref()
                && let Some(ident) = path.path.get_ident()
            {
                self.bump_param(&ident.to_string(), false);
            }
        }
    }

    /// Counts one use of the parameter named `name`; `field_read` marks a
    /// use that is only a field read.
    fn bump_param(&mut self, name: &str, field_read: bool) {
        if let Some(param) = self
            .facts
            .params
            .iter_mut()
            .find(|param| param.name == name)
        {
            param.uses += 1;
            if field_read {
                param.field_reads += 1;
            }
        }
    }

    /// Classifies a call path against the effect sink patterns.
    fn record_path_effect(&mut self, path: &syn::Path) {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let joined = segments.join("::");
        let effect = if joined.contains("fs::") || segments.first().is_some_and(|s| s == "File") {
            Some(Effect::FileSystem)
        } else if joined.contains("net::")
            || segments
                .iter()
                .any(|s| s == "TcpStream" || s == "TcpListener" || s == "UdpSocket")
        {
            Some(Effect::Network)
        } else if joined.contains("process::") || segments.first().is_some_and(|s| s == "Command")
        {
            Some(Effect::Process)
        } else if joined.contains("env::") {
            Some(Effect::Environment)
        } else {
            None
        };
        if let Some(effect) = effect {
            self.facts.effects.push(effect);
        }
    }
}

/// Infers the type name of a `let` initializer: a struct literal or a
/// `Type::new(..)` / `Type::default()` constructor call.
fn initializer_type(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Struct(literal) => literal
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let segments = &path.path.segments;
            if segments.len() < 2 {
                return None;
            }
            let method = segments.last()?.ident.to_string();
            if method != "new" && method != "default" {
                return None;
            }
            Some(segments[segments.len() - 2].ident.to_string())
        }
        syn::Expr::Reference(reference) => initializer_type(&reference.expr),
        _ => None
    }
}

impl<'ast> Visit<'ast> for FactCollector {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        let name = match &local.pat {
            syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
            syn::Pat::Type(typed) => {
                if let syn::Pat::Ident(pattern) = typed.pat.as_ref() {
                    if let Some(type_name) = type_last_segment(&typed.ty) {
                        self.param_types
                            .insert(pattern.ident.to_string(), type_name);
                    }
                    Some(pattern.ident.to_string())
                } else {
                    None
                }
            }
            _ => None
        };
        if let (Some(name), Some(init)) = (name, &local.init)
            && let Some(type_name) = initializer_type(&init.expr)
        {
            self.param_types.insert(name, type_name);
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.record_write_target(&assignment.left);
        self.visit_expr(&assignment.right);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if matches!(
            binary.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
                | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_)
                | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_)
                | syn::BinOp::ShrAssign(_)
        ) {
            self.record_write_target(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if let Some(key) = self.field_key(field) {
            self.facts.reads.push(key);
        }
        if let syn::Expr::Path(path) = field.base.as_ref()
            && let Some(ident) = path.path.get_ident()
        {
            let named = matches!(field.member, syn::Member::Named(_));
            self.bump_param(&ident.to_string(), named);
            return;
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if MUTATING_METHODS.contains(&method.as_str())
            && let syn::Expr::Field(field) = call.receiver.as_ref()
            && let Some(key) = self.field_key(field)
        {
            self.facts.writes.push(key);
        }
        if method == "clone" || method == "to_owned" || method == "to_string" {
            self.facts.clones += 1;
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_struct(&mut self, literal: &'ast syn::ExprStruct) {
        if let Some(segment) = literal.path.segments.last() {
            let name = segment.ident.to_string();
            let constructed = if name == "Self" {
                self.param_types.get("self").cloned().unwrap_or(name)
            } else {
                name
            };
            self.facts.constructs.push(constructed);
        }
        syn::visit::visit_expr_struct(self, literal);
    }

    fn visit_expr_unsafe(&mut self, block: &'ast syn::ExprUnsafe) {
        self.facts.effects.push(Effect::Unsafe);
        syn::visit::visit_expr_unsafe(self, block);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.path.segments.len() >= 2 {
            self.record_path_effect(&path.path);
            let first = path.path.segments[0].ident.to_string();
            let last = path.path.segments.last().map(|s| s.ident.to_string());
            if matches!(
                (first.as_str(), last.as_deref()),
                (
                    "Box" | "Vec" | "String" | "Rc" | "Arc",
                    Some("new" | "from" | "with_capacity")
                )
            ) {
                self.facts.allocations += 1;
            }
        } else if let Some(ident) = path.path.get_ident() {
            self.bump_param(&ident.to_string(), false);
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if let Some(ident) = invocation.path.get_ident() {
            let name = ident.to_string();
            if CONSOLE_MACROS.contains(&name.as_str()) {
                self.facts.effects.push(Effect::ConsoleIo);
            }
            if name == "vec" || name == "format" {
                self.facts.allocations += 1;
            }
        }
        syn::visit::visit_macro(self, invocation);
    }
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
