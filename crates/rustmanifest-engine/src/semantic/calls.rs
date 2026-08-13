// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace call graph: who calls whom, read from the identity a
//! resolver settled, and effect propagation over it.
//!
//! An edge is the compiler's answer and nothing else: a call site whose
//! reached body the resolution names binds to that body, one reaching a
//! body outside the workspace is a call out of it, and one no resolver
//! settled stays refused under its own name. The one reading left above
//! that is dispatch through the trait machinery, which is not a guess
//! about which body runs but the honest over-approximation that any
//! implementor's might.
//!
//! [`transitive_effects`] closes the direct per-function effect sets over
//! the graph to a fixpoint: a function that calls a filesystem-touching
//! function transitively touches the filesystem. Cycles (recursion) are
//! handled by the fixpoint iteration.

mod build;
mod context;
mod dispatch;
mod effects;
mod graph;
mod overlay;
mod reach;
mod rows;
mod tables;
mod target;

pub use self::{
    build::{call_graph, call_graph_rows, call_graph_rows_with, call_graph_with},
    context::{
        ResolutionContext, resolution_context, resolution_context_with, resolution_segments_with
    },
    effects::transitive_effects,
    graph::CallGraph,
    rows::RefusalRow
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

    use super::{call_graph, transitive_effects};
    use crate::{
        semantic::{
            behavior::{Effect, FunctionFacts, behavior_with},
            settling::{Settling, identity_of, settle_identity}
        },
        source::Source
    };

    /// The tree a resolver would state for exactly these mount points.
    fn mounted(rows: &[(&str, &str)]) -> rustmanifest_schema::Resolution {
        let mut settled = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            modules: rows
                .iter()
                .map(|(names, file)| rustmanifest_schema::ResolvedModule {
                    names:      (*names).to_owned(),
                    file:       PathBuf::from(*file),
                    crate_root: !names.contains("::"),
                    parent:     names.rsplit_once("::").map(|(above, _)| above.to_owned())
                })
                .collect(),
            ..rustmanifest_schema::Resolution::default()
        };
        settled.settle();
        settled
    }

    fn index_of(all: &[FunctionFacts], name: &str) -> usize {
        all.iter().position(|fact| fact.function == name).unwrap()
    }

    /// The facts of `text` and the graph over them, read with everything
    /// `settle` states a resolver settled about the fixture.
    ///
    /// A call binds to the body it reaches and to nothing else, so a test
    /// that expects an edge has to state which body that is. `settle`
    /// states what the compiler read off the written text — a type, a
    /// receiver, a field; the identity is settled beside it, and these
    /// fixtures declare every name once, so it is exactly the answer a
    /// compiler would give.
    fn settled_graph<'text>(
        text: &'text str,
        settle: impl FnOnce(Settling<'text>) -> Settling<'text>
    ) -> (Vec<FunctionFacts>, super::CallGraph) {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let mut resolution = settle(Settling::over("src/lib.rs", text)).done();
        crate::semantic::settling::settle_declared(&mut resolution, &sources);
        crate::semantic::settling::settle_standard_macros(&mut resolution, &sources);
        let facts = behavior_with(&sources, Some(&resolution));
        settle_identity(&mut resolution, &sources, &facts);
        let graph = call_graph(
            &facts,
            &super::resolution_context_with(&sources, Some(&resolution))
        );
        (facts, graph)
    }

    /// The same, where the only answer the compiler is asked for is the
    /// identity: which body each written call reaches.
    fn context_graph(text: &str) -> (Vec<FunctionFacts>, super::CallGraph) {
        settled_graph(text, |settling| settling)
    }

    /// Every refused candidate's row is the accounting's own answer, and
    /// the rows agree with the graph's counters by construction: the sum
    /// of each named row equals its counter exactly.
    ///
    /// Two rows are reachable. A call the resolver settled onto a body
    /// this workspace does not declare is foreign; a call it settled
    /// nothing about is unsettled, and that is the whole of what a refusal
    /// can mean once the reading stopped guessing.
    #[test]
    fn the_rows_agree_with_the_counters_by_construction() {
        let text = concat!(
            "pub struct A;\nimpl A {\n    pub fn run(&self) {}\n}\n",
            "pub fn go(a: &A) {\n    a.run();\n    std::fs::read(\"x\").ok();\n    ",
            "A::missing();\n}\n"
        );
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = Settling::over("src/lib.rs", text)
            .call("a.run()", "run", "()", Some("&A"))
            .reaching("std::fs::read(\"x\")", "std::fs::read")
            .done();
        let mut resolution = resolution;
        crate::semantic::settling::settle_declared(&mut resolution, &sources);
        crate::semantic::settling::settle_standard_macros(&mut resolution, &sources);
        let all = behavior_with(&sources, Some(&resolution));
        settle_identity(&mut resolution, &sources, &all);
        let (graph, rows) = super::call_graph_rows(
            &all,
            &super::resolution_context_with(&sources, Some(&resolution))
        );
        let count = |wanted: super::RefusalRow| -> u64 {
            rows.iter()
                .flatten()
                .filter(|(_, row)| *row == wanted)
                .count() as u64
        };
        assert_eq!(count(super::RefusalRow::Foreign), graph.external(), "{rows:?}");
        let total: usize = rows.iter().map(Vec::len).sum();
        assert_eq!(total as u64, graph.unresolved(), "{rows:?}");
        assert_eq!(graph.resolved(), 1, "{rows:?}");
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "A::run")]
        );
        assert!(
            rows.iter()
                .flatten()
                .any(|(candidate, row)| candidate == "A::missing"
                    && *row == super::RefusalRow::Unsettled),
            "{rows:?}"
        );
        assert!(
            rows.iter()
                .flatten()
                .any(|(candidate, row)| candidate == "fs::read"
                    && *row == super::RefusalRow::Foreign),
            "{rows:?}"
        );
    }

    /// An extension trait's default body is granted to every implementor
    /// of its supertrait, so a call written on such an implementor
    /// reaches that default — and a type the supertrait does not hold is
    /// granted nothing, however the call is spelled.
    #[test]
    fn ext_trait_defaults_dispatch_through_the_supertrait_bound() {
        let (all, graph) = settled_graph(
            "trait Base { fn poll(&self); }\nstruct Conn;\nimpl Base for Conn { fn poll(&self) { \
             std::net::TcpStream::connect(\"h\").ok(); } }\npub trait Ext: Base {\n    fn \
             write_all(&self) { self.poll(); }\n}\nstruct Plain;\nfn go(conn: Conn, other: \
             Plain) { conn.write_all(); other.write_all(); }",
            |settling| {
                settling
                    .call("conn.write_all()", "write_all", "()", Some("Conn"))
                    .call("other.write_all()", "write_all", "()", Some("Plain"))
            }
        );
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "Ext::write_all")],
            "the granted default is reached, and the outsider reaches nothing"
        );
    }

    /// A receiver the resolver renders as a trait object dispatches to
    /// every implementor's method: which one runs is not decided here,
    /// and naming all of them is the honest answer.
    #[test]
    fn dyn_reference_parameters_dispatch_too() {
        let (all, graph) = settled_graph(
            "trait Sink { fn write(&self); }\nstruct File;\nimpl Sink for File { fn write(&self) \
             {} }\nstruct Net;\nimpl Sink for Net { fn write(&self) {} }\nfn drain(sink: &dyn \
             Sink) { sink.write(); }",
            |settling| settling.call("sink.write()", "write", "()", Some("&dyn Sink"))
        );
        let callees = graph.callees(index_of(&all, "drain"));
        assert!(callees.contains(&index_of(&all, "File::write")));
        assert!(callees.contains(&index_of(&all, "Net::write")));
    }

    /// Library code that calls a method on an untyped receiver next to a
    /// crate whose only definition of that name lives in its test target.
    fn library_and_test_target() -> [Source; 2] {
        [
            Source::new(
                PathBuf::from("crates/demo/src/lib.rs"),
                "pub struct Conn;\nimpl Conn { pub fn read_head(&self, headers: Headers) { \
                 headers.get(TE); } }"
                    .to_owned()
            ),
            Source::new(
                PathBuf::from("crates/demo/tests/server.rs"),
                "struct TestClient;\nimpl TestClient { fn get(&self) { \
                 std::net::TcpStream::connect(\"h\").ok(); } }\nfn drive(client: TestClient) { \
                 client.get(); }"
                    .to_owned()
            )
        ]
    }

    #[test]
    fn a_library_call_never_binds_to_a_test_target_definition() {
        let sources = library_and_test_target();
        let all = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let graph = call_graph(&all, &super::resolution_context(&sources));
        let caller = index_of(&all, "Conn::read_head");
        assert!(
            graph.callees(caller).is_empty(),
            "the only workspace `get` compiles into a different target"
        );
        assert!(!transitive_effects(&all, &graph)[caller].contains(&Effect::Network));
    }

    #[test]
    fn a_call_inside_one_test_target_still_resolves() {
        let sources = library_and_test_target();
        let resolution = Settling::over("crates/demo/src/lib.rs", sources[0].text())
            .and_over("crates/demo/tests/server.rs", sources[1].text())
            .call("client.get()", "get", "()", Some("TestClient"))
            .done();
        let mut resolution = resolution;
        crate::semantic::settling::settle_declared(&mut resolution, &sources);
        crate::semantic::settling::settle_standard_macros(&mut resolution, &sources);
        let all = behavior_with(&sources, Some(&resolution));
        settle_identity(&mut resolution, &sources, &all);
        let graph = call_graph(
            &all,
            &super::resolution_context_with(&sources, Some(&resolution))
        );
        let caller = index_of(&all, "drive");
        assert!(
            graph
                .callees(caller)
                .contains(&index_of(&all, "TestClient::get"))
        );
        assert!(transitive_effects(&all, &graph)[caller].contains(&Effect::Network));
    }

    #[test]
    fn two_integration_test_files_are_two_crates_that_cannot_call_each_other() {
        let sources = [
            Source::new(
                PathBuf::from("crates/demo/tests/one.rs"),
                "struct Client;\nimpl Client { fn get(&self) { \
                 std::net::TcpStream::connect(\"h\").ok(); } }"
                    .to_owned()
            ),
            Source::new(
                PathBuf::from("crates/demo/tests/two.rs"),
                "fn drive(client: Other) { client.get(); }".to_owned()
            )
        ];
        let all = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let graph = call_graph(&all, &super::resolution_context(&sources));
        assert!(graph.callees(index_of(&all, "drive")).is_empty());
    }

    #[test]
    fn scip_overlay_adds_only_edges_the_candidates_missed() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "struct Wallet;\nimpl Wallet { fn credit(&self) {} }\nfn helper() {}\nfn caller() { \
             helper(); }"
                .to_owned()
        )];
        let all = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let settled = identity_of(&sources, &all);
        let context = super::resolution_context_with(&sources, Some(&settled));
        let caller = index_of(&all, "caller");
        let mut overlay = vec![Vec::new(); all.len()];
        overlay[caller] = vec![index_of(&all, "Wallet::credit"), index_of(&all, "helper")];
        let graph = super::call_graph_with(&all, &context, Some(&overlay));
        let callees = graph.callees(caller);
        assert!(callees.contains(&index_of(&all, "Wallet::credit")));
        assert!(callees.contains(&index_of(&all, "helper")));
        assert_eq!(graph.scip_resolved(), 1);
        assert!(graph.indexed(caller, index_of(&all, "Wallet::credit")));
        assert!(
            !graph.indexed(caller, index_of(&all, "helper")),
            "an edge the resolution already settled keeps its own provenance"
        );
        let plain = call_graph(&all, &context);
        assert_eq!(plain.scip_resolved(), 0);
        assert!(!plain.indexed(caller, index_of(&all, "Wallet::credit")));
    }

    #[test]
    fn free_function_calls_resolve_by_bare_name() {
        let (all, graph) = context_graph("fn helper() {}\nfn caller() { helper(); }");
        assert_eq!(
            graph.callees(index_of(&all, "caller")),
            &[index_of(&all, "helper")]
        );
        assert_eq!(graph.resolved(), 1);
    }

    /// The type holding a field and the type that field carries are the
    /// resolver's answers, and the call written on the field is named by
    /// them.
    #[test]
    fn field_receivers_resolve_through_struct_definitions() {
        let (all, graph) = settled_graph(
            "struct Engine;\nimpl Engine { fn start(&self) {} }\nstruct Car { engine: Engine \
             }\nimpl Car { fn drive(&self) { self.engine.start(); } }",
            |settling| {
                settling
                    .field("self.engine", "Car", "Engine")
                    .typed("self.engine", "Engine")
            }
        );
        assert_eq!(
            graph.callees(index_of(&all, "Car::drive")),
            &[index_of(&all, "Engine::start")]
        );
    }

    #[test]
    fn associated_calls_resolve_by_qualified_name() {
        let (all, graph) = context_graph(
            "struct Board;\nimpl Board { fn reset() {} }\nfn wipe() { Board::reset(); }"
        );
        assert_eq!(
            graph.callees(index_of(&all, "wipe")),
            &[index_of(&all, "Board::reset")]
        );
    }

    /// The type of a receiver is the compiler's to say, and the member
    /// call written on it is named by that type.
    #[test]
    fn method_calls_resolve_through_typed_receivers() {
        let (all, graph) = settled_graph(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn balance(&self) -> i64 { \
             self.balance } }\nfn audit(wallet: &Wallet) -> i64 { wallet.balance() }",
            |settling| settling.call("wallet.balance()", "balance", "i64", Some("&Wallet"))
        );
        assert_eq!(
            graph.callees(index_of(&all, "audit")),
            &[index_of(&all, "Wallet::balance")]
        );
    }

    /// A receiver no resolver typed settles nothing: the workspace
    /// carrying the written member name is not an answer to which body
    /// runs.
    #[test]
    fn an_untyped_receiver_stays_unresolved() {
        let (all, graph) = context_graph(
            "struct A;\nimpl A { fn go(&self) {} }\nstruct B;\nimpl B { fn go(&self) {} }\nfn \
             run(x: u32) { x.go(); }"
        );
        assert!(graph.callees(index_of(&all, "run")).is_empty());
        assert_eq!(graph.unresolved(), 1);
    }

    #[test]
    fn effects_propagate_transitively() {
        let (all, graph) = context_graph(
            "fn log(text: &str) { println!(\"{text}\"); }\nfn middle(text: &str) { log(text); \
             }\nfn top(text: &str) { middle(text); }"
        );
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "top")], vec![Effect::ConsoleIo]);
        assert_eq!(effects[index_of(&all, "middle")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn recursion_reaches_a_fixpoint() {
        let (all, graph) = context_graph(
            "fn ping(n: u32) { if n > 0 { pong(n - 1); } }\nfn pong(n: u32) { println!(\"{n}\"); \
             ping(n); }"
        );
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "ping")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn external_calls_are_honestly_unresolved() {
        let (all, graph) =
            context_graph("fn read() -> String { std::fs::read_to_string(\"x\").unwrap() }");
        assert_eq!(graph.resolved(), 0);
        assert!(graph.unresolved() > 0);
        assert!(graph.callees(index_of(&all, "read")).is_empty());
    }

    #[test]
    fn cross_crate_calls_resolve_through_crate_names() {
        let sources = [
            Source::new(
                PathBuf::from("crates/rusty-core/src/lib.rs"),
                "pub fn helper() { println!(\"x\"); }".to_owned()
            ),
            Source::new(
                PathBuf::from("crates/rusty-app/src/lib.rs"),
                "pub fn run() { rusty_core::helper(); }".to_owned()
            )
        ];
        let mut settled = mounted(&[
            ("rusty_core", "crates/rusty-core/src/lib.rs"),
            ("rusty_app", "crates/rusty-app/src/lib.rs")
        ]);
        let all = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        settle_identity(&mut settled, &sources, &all);
        let graph = call_graph(
            &all,
            &super::resolution_context_with(&sources, Some(&settled))
        );
        assert_eq!(
            graph.callees(index_of(&all, "run")),
            &[index_of(&all, "helper")]
        );
    }
}
