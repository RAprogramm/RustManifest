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
//!
//! Each stage of the extraction sits in its own submodule beside this
//! file: the workspace-wide typing tables, the import table of one file,
//! the declaration context of one item, the body walk itself, the
//! conversions an error derive generates, the facts of a trait's default
//! bodies, and the assembly that drives them all.

mod assembly;
mod choice;
mod collector;
mod conversion;
mod declaration;
mod exit;
mod extraction;
mod folding;
mod handback;
mod imports;
mod input;
mod operation;
mod output;
mod query;
mod rendering;
mod scoped;
mod signature;
mod tables;
mod targeting;
mod trait_default;
mod types;
mod value;

pub(in crate::semantic) use self::assembly::outermost as outermost_name;
pub use self::{
    assembly::{behavior, behavior_with},
    choice::{Alternative, CHOICE_SET, Choice, Closure},
    exit::{ExitRoute, ExitStatus, ProcessExit},
    handback::{HANDBACK_SET, Handback, Outcome},
    input::{
        Channel, INPUT_SOURCE_SET, InputRead, Intake, ReadShape, Selector, SelectorGap, Terminator
    },
    operation::{
        Arithmetic, Comparison, Conversion, Guarding, Junction, OPERATION_SET, Operation,
        OperationSite
    },
    output::{
        Acquisition, Buffering, Conduit, Grade, Guard, Hole, HoleSource, Multiplicity, Newline,
        OUTPUT_SINK_SET, OpaqueReason, OutputWrite, Payload, Segment, SiteGuard, Stream
    },
    query::{QuerySite, Statement, StatementCoverage, statement_coverage},
    types::{CallSite, Effect, EffectSite, FunctionFacts, FunctionFlags, ParamFacts, ParamMode},
    value::{
        Arguments, Condition, Disposal, Literal, OriginGap, ParamSlot, Provenance,
        VALUE_ORIGIN_SET
    }
};

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
    use crate::{semantic::settling::Settling, source::Source};

    fn facts(text: &str) -> Vec<FunctionFacts> {
        behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
    }

    /// The facts of `text` read with a resolver's answer that the
    /// expression spelled `expression` has type `rendered`.
    fn facts_typed(text: &str, expression: &str, rendered: &str) -> Vec<FunctionFacts> {
        facts_settled(text, |settling| settling.typed(expression, rendered))
    }

    /// The facts of `text` read with everything `settle` states a
    /// resolver settled about it.
    fn facts_settled<'text>(
        text: &'text str,
        settle: impl FnOnce(Settling<'text>) -> Settling<'text>
    ) -> Vec<FunctionFacts> {
        let resolution = settle(Settling::over("src/lib.rs", text)).done();
        super::behavior_with(
            &[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())],
            Some(&resolution)
        )
    }

    /// The compiler reads a receiver's type from the impl it is written
    /// in; the text says only `self`, so a reading of the text cannot.
    #[test]
    fn a_settled_signature_types_the_receiver_the_text_leaves_bare() {
        let text = "struct Wallet;\nimpl Wallet { fn peek(&self) {} }\n";
        let start = u32::try_from(text.find("peek").unwrap()).unwrap();
        let mut settled = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            signatures: vec![rustmanifest_schema::ResolvedSignature {
                at:      rustmanifest_schema::Location {
                    file:  PathBuf::from("src/lib.rs"),
                    range: rustmanifest_schema::Range {
                        start,
                        end: start + 4
                    }
                },
                params:  vec![rustmanifest_schema::ResolvedParam {
                    name:  "self".to_owned(),
                    named: "&Wallet".to_owned(),
                    mode:  rustmanifest_schema::ResolvedMode::Borrowed
                }],
                returns: "()".to_owned()
            }],
            ..rustmanifest_schema::Resolution::default()
        };
        settled.settle();
        let all = super::behavior_with(
            &[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())],
            Some(&settled)
        );
        let peek = named(&all, "Wallet::peek");
        assert_eq!(peek.params.len(), 1);
        assert_eq!(peek.params[0].type_name, "Wallet");
        assert_eq!(peek.params[0].mode, super::ParamMode::Borrowed);
    }

    /// A field hanging off an expression the written text cannot type is
    /// unnamed on its own, and named the moment a resolver states the
    /// type — with nothing in between reading how it was spelled.
    #[test]
    fn a_type_the_resolver_settled_names_a_field_the_reading_leaves_unnamed() {
        let text = "fn go(outer: Outer) { outer.inner.count = 1; }";
        let read = facts(text);
        assert_eq!(named(&read, "go").writes, vec!["?.count".to_owned()]);

        let all = facts_typed(text, "outer.inner", "Inner");
        assert_eq!(named(&all, "go").writes, vec!["Inner.count".to_owned()]);
    }

    fn named<'a>(all: &'a [FunctionFacts], name: &str) -> &'a FunctionFacts {
        all.iter().find(|facts| facts.function == name).unwrap()
    }

    /// Both spellings of a `sqlx` statement — the macro and the
    /// function — record what the statement does and which table its
    /// own grammar slot names; a statement naming no single table keeps
    /// its record with the table left open, so the count of statements
    /// never silently drops below the count of database effects.
    #[test]
    fn written_statements_record_their_kind_and_their_table() {
        let all = facts(
            "async fn create(pool: &sqlx::PgPool, title: String) {\n    sqlx::query!(\"INSERT \
             INTO tasks (title) VALUES ($1)\", title);\n}\nasync fn list(pool: &sqlx::PgPool) \
             {\n    sqlx::query(\"SELECT id, title FROM tasks\");\n}\nasync fn report(pool: \
             &sqlx::PgPool) {\n    sqlx::query(\"SELECT t.id FROM tasks t JOIN users u ON TRUE\");\
             \n}"
        );
        let one = |name: &str| {
            let sites = &named(&all, name).queries;
            assert_eq!(sites.len(), 1, "{sites:?}");
            sites[0].clone()
        };
        let created = one("create");
        assert_eq!(created.statement, super::Statement::Inserts);
        assert_eq!(created.table.as_deref(), Some("tasks"));
        let listed = one("list");
        assert_eq!(listed.statement, super::Statement::Selects);
        assert_eq!(listed.table.as_deref(), Some("tasks"));
        let joined = one("report");
        assert_eq!(joined.table, None, "a join must not claim one table");
    }

    /// A statement built at runtime proves the database effect with its
    /// kind unproven: the constructor call is the proof, the statement
    /// channel stays empty, and the honesty counter shows the gap — a
    /// body that provably sends a statement must not read as pure.
    #[test]
    fn a_runtime_statement_proves_the_effect_without_a_kind() {
        let all =
            facts("async fn purge(pool: &sqlx::PgPool, sql: &str) {\n    sqlx::query(sql);\n}");
        let held = named(&all, "purge");
        assert!(
            held.effects.contains(&super::Effect::Database),
            "{:?}",
            held.effects
        );
        assert!(
            held.effect_sites
                .iter()
                .any(|site| site.effect == super::Effect::Database && site.proof == "sqlx::query"),
            "{:?}",
            held.effect_sites
        );
        assert!(held.queries.is_empty(), "{:?}", held.queries);
        let coverage = super::query::statement_coverage(std::slice::from_ref(held));
        assert_eq!(coverage.effect_sites, 1, "{coverage:?}");
        assert_eq!(coverage.kinds_proven, 0, "{coverage:?}");
    }

    /// A name captured inline in a template is a use of the parameter it
    /// names, counted where the capture is read — no path visit ever
    /// sees it.
    /// The function form hands its arguments over one `bind` at a time,
    /// and the chain lands on the statement site in written order; a
    /// computed argument claims nothing.
    #[test]
    fn a_bind_chain_lands_its_plain_names_on_the_statement_in_written_order() {
        let all = facts(
            "async fn store(pool: &sqlx::PgPool, id: i64, title: String) {\n    \
             sqlx::query(\"INSERT INTO tasks (id, title) VALUES ($1, $2)\")\n        \
             .bind(id)\n        .bind(&title)\n        .bind(title.len())\n        \
             .execute(pool)\n        .await;\n}"
        );
        let sites = &named(&all, "store").queries;
        assert_eq!(sites.len(), 1, "{sites:?}");
        assert_eq!(sites[0].binds, vec!["id".to_owned(), "title".to_owned()]);
    }

    /// The counter states the channel's own blind spot: a DDL statement
    /// proves the effect and nothing further, so the effect count
    /// exceeds the proven-kind count by exactly that site.
    #[test]
    fn the_statement_counter_names_the_channels_blind_spot() {
        let all = facts(
            "async fn migrate(pool: &sqlx::PgPool) {\n    sqlx::query(\"CREATE TABLE tasks (id \
             BIGINT)\");\n}\nasync fn list(pool: &sqlx::PgPool) {\n    sqlx::query(\"SELECT id \
             FROM tasks\");\n}\nasync fn report(pool: &sqlx::PgPool) {\n    sqlx::query(\"SELECT \
             t.id FROM tasks t JOIN users u ON TRUE\");\n}"
        );
        let counted = super::statement_coverage(&all);
        assert_eq!(counted.effect_sites, 3);
        assert_eq!(counted.kinds_proven, 2, "the DDL site proves no kind");
        assert_eq!(counted.tables_named, 1, "the join names no table");
    }

    /// The type holding `self.balance` is the impl's, which the written
    /// text spells nowhere; the resolver states it, and the write and the
    /// read are named by it.
    #[test]
    fn self_field_writes_and_reads_are_qualified() {
        let all = facts_settled(
            "struct Wallet { balance: i64 }\nimpl Wallet {\n    fn credit(&mut self, amount: \
             i64) { self.balance += amount; }\n    fn balance(&self) -> i64 { self.balance }\n}",
            |settling| {
                settling
                    .after("fn credit")
                    .field("self.balance", "Wallet", "i64")
                    .after("fn balance")
                    .field("self.balance", "Wallet", "i64")
            }
        );
        let credit = named(&all, "Wallet::credit");
        assert_eq!(credit.writes, vec!["Wallet.balance"]);
        assert_eq!(credit.reads, vec!["Wallet.balance"]);
        let getter = named(&all, "Wallet::balance");
        assert_eq!(getter.reads, vec!["Wallet.balance"]);
        assert!(getter.writes.is_empty());
    }

    /// A field the resolver settled an owner for is named by that owner;
    /// one it settled nothing for keeps its honest `?`.
    #[test]
    fn a_settled_owner_names_the_field_written_on_it() {
        let all = facts_settled(
            "struct Wallet { balance: i64 }\nfn drain(wallet: &mut Wallet) { wallet.balance = 0; \
             }\nfn peek(other: &mut Wallet) { other.balance = 1; }",
            |settling| settling.field("wallet.balance", "Wallet", "i64")
        );
        assert_eq!(named(&all, "drain").writes, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "peek").writes, vec!["?.balance"]);
    }

    #[test]
    fn unresolvable_owner_is_marked_honestly() {
        let all =
            facts("fn poke(tuple: (u8, u8)) { tuple.0; }\nfn chain() { helper().field = 1; }");
        assert_eq!(named(&all, "chain").writes, vec!["?.field"]);
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

    /// A generic field has no type the written text can name — the
    /// resolver names it, and the call written on it is qualified by that
    /// name. An `await` on a call the resolver typed reaches the `poll`
    /// that type declares.
    #[test]
    fn a_settled_receiver_qualifies_the_call_and_awaits_reach_poll() {
        let all = facts_settled(
            "struct WriteAll<W> { writer: W }\nimpl<W: AsyncWrite> Future for WriteAll<W> {\n    \
             fn poll(&mut self) -> u8 { self.writer.poll_write(); 0 }\n}\nstruct Done;\nimpl \
             Done { fn write_all(&self) -> WriteAll<Done> { WriteAll { writer: Done } } }\nasync \
             fn drive(done: Done) { done.write_all().await; }",
            |settling| {
                settling
                    .typed("self.writer", "AsyncWrite")
                    .call("done.write_all()", "write_all", "WriteAll<Done>", Some("Done"))
            }
        );
        let poll = named(&all, "WriteAll::poll");
        assert!(poll.calls.contains(&"AsyncWrite::poll_write".to_owned()));
        let drive = named(&all, "drive");
        assert!(drive.calls.contains(&"WriteAll::poll".to_owned()));
    }

    /// The error half a `?` converts is read out of the rendering the
    /// compiler gives the tried call, so a site whose enclosing signature
    /// declares another failure type records the conversion the language
    /// performs there.
    #[test]
    fn try_sites_record_the_implicit_conversion() {
        let all = facts_settled(
            "struct AppError;\nstruct DbError;\nfn load() -> Result<u8, DbError> { Ok(0) }\nfn \
             handle() -> Result<u8, AppError> { let v = load()?; Ok(v) }\nfn same() -> \
             Result<u8, DbError> { let v = load()?; Ok(v) }",
            |settling| settling.all_typed("load()", "Result<u8, DbError>")
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

    /// A tuple field is written as a position and nothing else; the
    /// resolver states what stands at it, and the call written on it
    /// carries that type's effect.
    #[test]
    fn a_settled_tuple_field_types_its_receiver() {
        let all = facts_settled(
            "struct Wire(TcpStream);\nimpl Wire { fn pump(&self) { self.0.read(&mut []).ok(); } \
             }\nstruct Layer(Wrapper<TcpStream>);\nimpl Layer { fn go(&self) { \
             self.0.read(&mut []).ok(); } }",
            |settling| settling.typed("self.0", "TcpStream")
        );
        assert_eq!(named(&all, "Wire::pump").effects, vec![Effect::Network]);
        assert!(named(&all, "Layer::go").effects.is_empty());
    }

    #[test]
    fn tuple_field_reads_stay_out_of_the_fact_channel() {
        let all = facts("fn poke(pair: (u8, u8)) -> u8 { pair.0 }");
        assert!(named(&all, "poke").reads.is_empty());
    }

    /// An effect category on its own is an assertion; the site says what
    /// proved it, so a reader can go to the byte range and check.
    #[test]
    fn an_effect_cites_the_path_that_proved_it() {
        let all = facts(
            "fn serve() { std::net::TcpListener::bind(\"0.0.0.0:80\").ok(); }\nfn configure() { \
             std::env::var(\"PORT\").ok(); }"
        );
        let serve = named(&all, "serve");
        assert_eq!(serve.effects, vec![Effect::Network]);
        assert_eq!(serve.effect_sites.len(), 1);
        assert_eq!(serve.effect_sites[0].effect, Effect::Network);
        assert_eq!(serve.effect_sites[0].proof, "std::net::TcpListener::bind");
        assert!(serve.effect_sites[0].range.end > serve.effect_sites[0].range.start);

        let configure = named(&all, "configure");
        assert_eq!(configure.effects, vec![Effect::Environment]);
        assert_eq!(configure.effect_sites[0].proof, "std::env::var");
    }

    #[test]
    fn an_unsafe_block_cites_itself() {
        let all = facts("fn poke() { unsafe { } }");
        let sites = &named(&all, "poke").effect_sites;
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].effect, Effect::Unsafe);
        assert_eq!(sites[0].proof, "unsafe");
    }

    #[test]
    fn a_function_touching_nothing_cites_nothing() {
        let all = facts("fn add(left: u8, right: u8) -> u8 { left + right }");
        assert!(named(&all, "add").effect_sites.is_empty());
    }

    /// A resolution names two files' same-named types apart, and the
    /// effect each body reaches follows the name it was given rather than
    /// whichever declaration stood nearer.
    #[test]
    fn a_resolution_tells_two_same_named_types_apart() {
        let net_text = "pub struct Client { inner: TcpStream }\nimpl Client { pub fn go(&self) { \
                        self.inner.read(&mut []).ok(); } }";
        let disk_text = "pub struct Client { inner: File }\nimpl Client { pub fn go(&self) { \
                         self.inner.read(&mut []).ok(); } }";
        let resolution = Settling::over("app/src/net.rs", net_text)
            .typed("self.inner", "TcpStream")
            .and_over("app/src/disk.rs", disk_text)
            .typed("self.inner", "File")
            .done();
        let sources = vec![
            Source::new(PathBuf::from("app/src/net.rs"), net_text.to_owned()),
            Source::new(PathBuf::from("app/src/disk.rs"), disk_text.to_owned()),
        ];
        let all = super::behavior_with(&sources, Some(&resolution));
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
    fn std_method_axioms_fire_on_typed_receivers() {
        let all = facts_settled(
            "struct Log { sink: File }\nimpl Log { fn flush(&self) { \
             self.sink.sync_all().ok(); } }\nfn pump(stream: &mut TcpStream) { \
             stream.read(&mut []).ok(); }\nfn untyped(stream: &mut Unknown) { \
             stream.read(&mut []).ok(); }",
            |settling| {
                settling
                    .typed("self.sink", "File")
                    .after("fn pump(stream: &mut TcpStream) { ")
                    .typed("stream", "&mut TcpStream")
            }
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

    /// What a `let` holds decides whether writing through it assembles a
    /// value or changes one that was already standing: a literal and a
    /// constructor call both leave a fresh value, an annotated binding
    /// taken from elsewhere does not. The owner naming each key is the
    /// resolver's.
    #[test]
    fn let_bindings_resolve_owners_by_annotation_literal_and_constructor() {
        let all = facts_settled(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn new() -> Self { Self { balance: 0 \
             } } }\nfn annotated() { let wallet: Wallet = make(); wallet.balance = 1; }\nfn \
             literal() { let wallet = Wallet { balance: 0 }; wallet.balance = 2; }\nfn \
             constructed() { let wallet = Wallet::new(); wallet.balance = 3; }\nfn opaque() { \
             let wallet = make(); wallet.balance = 4; }",
            |settling| {
                settling
                    .after("fn annotated")
                    .field("wallet.balance", "Wallet", "i64")
                    .after("fn literal")
                    .field("wallet.balance", "Wallet", "i64")
                    .after("fn constructed")
                    .field("wallet.balance", "Wallet", "i64")
            }
        );
        assert_eq!(named(&all, "annotated").writes, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "literal").builds, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "constructed").builds, vec!["Wallet.balance"]);
        assert_eq!(named(&all, "opaque").writes, vec!["?.balance"]);
    }

    /// A binding taken from an associated factory holds a value this body
    /// made, so writing through it assembles rather than changes.
    #[test]
    fn associated_factories_leave_a_fresh_binding() {
        let all = facts_settled(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn open() -> Self { Self { balance: 0 \
             } } }\nfn opened() { let wallet = Wallet::open(); wallet.balance = 1; }",
            |settling| {
                settling
                    .field("wallet.balance", "Wallet", "i64")
                    .call("Wallet::open()", "open", "Wallet", None)
            }
        );
        assert_eq!(named(&all, "opened").builds, vec!["Wallet.balance"]);
    }

    /// A field written through a name the body itself made is assembly,
    /// not a change to state someone else holds — and what the name holds
    /// is the type the compiler settled for the call that made it.
    #[test]
    fn assembling_a_fresh_value_is_a_build_not_a_write() {
        let all = facts_settled(
            "struct Order { items: u32 }\nimpl Order { fn load() -> Self { Self { items: 0 } } \
             }\nstruct Repo;\nimpl Repo { fn get_order(&self) -> Order { let mut order = \
             Order::load(); order.items = 3; order } }",
            |settling| {
                settling
                    .field("order.items", "Order", "u32")
                    .call("Order::load()", "load", "Order", None)
            }
        );
        let getter = named(&all, "Repo::get_order");
        assert!(getter.writes.is_empty());
        assert_eq!(getter.builds, vec!["Order.items"]);
    }

    #[test]
    fn rebinding_a_fresh_name_to_state_restores_write_facts() {
        let all = facts_settled(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn new() -> Self { Self { balance: 0 \
             } } }\nfn rebind(source: &mut Wallet) { let wallet = Wallet::new(); let wallet = \
             source; wallet.balance = 1; }",
            |settling| settling.field("wallet.balance", "Wallet", "i64")
        );
        let facts = named(&all, "rebind");
        assert!(facts.builds.is_empty());
        assert_eq!(facts.writes, vec!["Wallet.balance"]);
    }

    /// A pack states what a crate's own method reaches; which type the
    /// method was called on is the resolver's answer, and the effect
    /// follows from the two together.
    #[test]
    fn knowledge_packs_carry_the_effects_of_settled_receivers() {
        let all = facts_settled(
            "async fn fetch(client: &Client) -> Result<String, String> { let body = \
             client.get(\"u\").send().await.unwrap().text().await.unwrap(); Ok(body) }\nfn \
             probe() { let dir = tempfile::tempdir().unwrap(); dir.close().unwrap(); }",
            |settling| {
                settling
                    .after("let body = ")
                    .typed("client", "&Client")
                    .call(
                        "client.get(\"u\").send()",
                        "send",
                        "Response",
                        Some("RequestBuilder")
                    )
                    .after("tempfile::tempdir().unwrap(); ")
                    .typed("dir", "TempDir")
            }
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
