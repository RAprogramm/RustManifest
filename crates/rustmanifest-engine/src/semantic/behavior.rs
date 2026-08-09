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
mod returned;
mod scoped;
mod signature;
mod tables;
mod targeting;
mod trait_default;
mod types;
mod value;

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
        Acquisition, Conduit, Grade, Guard, Hole, HoleSource, Multiplicity, Newline,
        OUTPUT_SINK_SET, OpaqueReason, OutputWrite, Payload, Segment, SiteGuard, Stream
    },
    query::{QuerySite, Statement, StatementCoverage, statement_coverage},
    types::{CallSite, Effect, EffectSite, FunctionFacts, FunctionFlags, ParamFacts, ParamMode},
    value::{
        Arguments, Condition, Disposal, Literal, OriginGap, ParamSlot, Provenance,
        VALUE_ORIGIN_SET
    }
};
use self::{returned::Returned, scoped::Scoped};

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
