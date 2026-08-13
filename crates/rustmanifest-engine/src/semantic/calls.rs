// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace call graph: who calls whom, resolved from behavioral call
//! candidates, and effect propagation over it.
//!
//! Resolution is conservative and honest: a candidate produces an edge
//! only when it matches exactly one known function — a qualified
//! `Type::method` by full name, a bare `name` against free functions, an
//! unqualified `?::method` against method names workspace-wide. Ambiguous
//! and external candidates are counted as unresolved, never guessed.
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
        semantic::behavior::{Effect, behavior},
        source::Source
    };

    /// Every refused candidate's row is the accounting's own answer,
    /// and the rows agree with the graph's counters by construction:
    /// the sum of each named row equals its counter exactly.
    #[test]
    fn the_rows_agree_with_the_counters_by_construction() {
        let text = concat!(
            "pub struct A;\nimpl A {\n    pub fn run(&self) {}\n}\n",
            "pub struct B;\nimpl B {\n    pub fn run(&self) {}\n}\n",
            "pub fn go(a: &A) {\n    a.run();\n    std::fs::read(\"x\").ok();\n    ",
            "A::missing();\n}\n"
        );
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let all = behavior(&sources);
        let context = crate::semantic::calls::resolution_context(&sources);
        let (graph, rows) = super::call_graph_rows(&all, &context);
        let count = |wanted: super::RefusalRow| -> u64 {
            rows.iter()
                .flatten()
                .filter(|(_, row)| *row == wanted)
                .count() as u64
        };
        assert_eq!(count(super::RefusalRow::Foreign), graph.external());
        assert_eq!(
            count(super::RefusalRow::UnknownReceiver),
            graph.unknown_receiver()
        );
        assert_eq!(count(super::RefusalRow::Ambiguous), graph.ambiguous());
        assert_eq!(
            count(super::RefusalRow::OwnerWithoutMember),
            graph.owned_unmatched()
        );
        let total: usize = rows.iter().map(Vec::len).sum();
        assert_eq!(total as u64, graph.unresolved(), "{rows:?}");
        assert!(
            rows.iter()
                .flatten()
                .any(|(candidate, row)| candidate == "A::missing"
                    && *row == super::RefusalRow::OwnerWithoutMember),
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

    fn facts(text: &str) -> Vec<crate::semantic::behavior::FunctionFacts> {
        behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
    }

    fn index_of(all: &[crate::semantic::behavior::FunctionFacts], name: &str) -> usize {
        all.iter().position(|fact| fact.function == name).unwrap()
    }

    fn context_graph(
        text: &str
    ) -> (
        Vec<crate::semantic::behavior::FunctionFacts>,
        super::CallGraph
    ) {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let graph = call_graph(&facts, &super::resolution_context(&sources));
        (facts, graph)
    }

    /// The same, with what a resolver settled about the types of the
    /// expressions named by `typed` — each a written fragment and the
    /// type the resolver states it has.
    fn context_graph_settled(
        text: &str,
        typed: &[(&str, &str)]
    ) -> (
        Vec<crate::semantic::behavior::FunctionFacts>,
        super::CallGraph
    ) {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let mut resolution = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            types: typed
                .iter()
                .map(|(written, rendered)| {
                    let start =
                        u32::try_from(text.find(written).expect("the fragment is written"))
                            .expect("the fixture is small");
                    rustmanifest_schema::ResolvedType {
                        at:       rustmanifest_schema::Location {
                            file:  PathBuf::from("src/lib.rs"),
                            range: rustmanifest_schema::Range {
                                start,
                                end: start
                                    + u32::try_from(written.len()).expect("the fixture is small")
                            }
                        },
                        rendered: (*rendered).to_owned()
                    }
                })
                .collect(),
            ..rustmanifest_schema::Resolution::default()
        };
        resolution.settle();
        let facts = crate::semantic::behavior::behavior_with(&sources, Some(&resolution));
        let graph = call_graph(&facts, &super::resolution_context(&sources));
        (facts, graph)
    }

    #[test]
    fn generic_bound_parameters_dispatch_over_implementors() {
        let (all, graph) = context_graph(
            "trait Sink { fn write(&self); }\nstruct Disk;\nimpl Sink for Disk { fn write(&self) \
             {} }\nstruct Wire;\nimpl Sink for Wire { fn write(&self) {} }\nfn drain<S: \
             Sink>(sink: S) { sink.write(); }"
        );
        let callees = graph.callees(index_of(&all, "drain"));
        assert!(callees.contains(&index_of(&all, "Disk::write")));
        assert!(callees.contains(&index_of(&all, "Wire::write")));
    }

    #[test]
    fn ext_trait_defaults_dispatch_through_the_supertrait_bound() {
        let (all, graph) = context_graph(
            "trait Base { fn poll(&self); }\nstruct Conn;\nimpl Base for Conn { fn poll(&self) { \
             std::net::TcpStream::connect(\"h\").ok(); } }\npub trait Ext: Base {\n    fn \
             write_all(&self) { self.poll(); }\n}\nstruct Plain;\nfn go(conn: Conn, other: \
             Plain) { conn.write_all(); other.write_all(); }"
        );
        let callees = graph.callees(index_of(&all, "go"));
        assert!(callees.contains(&index_of(&all, "Ext::write_all")));
        let effects = super::transitive_effects(&all, &graph);
        assert!(effects[index_of(&all, "go")].contains(&Effect::Network));
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
        let all = behavior(&sources);
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
        let all = behavior(&sources);
        let graph = call_graph(&all, &super::resolution_context(&sources));
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
        let all = behavior(&sources);
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
        let all = behavior(&sources);
        let context = super::resolution_context(&sources);
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
            "an edge the candidates already produced keeps its syntactic provenance"
        );
        let plain = call_graph(&all, &context);
        assert_eq!(plain.scip_resolved(), 0);
        assert!(!plain.indexed(caller, index_of(&all, "Wallet::credit")));
    }

    /// A candidate matching several linkable functions binds when the
    /// index confirms exactly one of them — the uniqueness gate admits
    /// the single confirmed match — and stays refused when the index
    /// confirms both, because two confirmations settle nothing.
    #[test]
    fn the_index_settles_an_ambiguous_candidate_to_its_one_confirmed_match() {
        let sources = [
            Source::new(
                PathBuf::from("src/one.rs"),
                "pub fn fetch() {}\n".to_owned()
            ),
            Source::new(
                PathBuf::from("src/two.rs"),
                "pub fn fetch() {}\n".to_owned()
            ),
            Source::new(
                PathBuf::from("src/lib.rs"),
                "mod one;\nmod two;\nfn caller() { fetch(); }\n".to_owned()
            )
        ];
        let all = behavior(&sources);
        let context = super::resolution_context(&sources);
        let caller = index_of(&all, "caller");
        let matched: Vec<usize> = all
            .iter()
            .enumerate()
            .filter_map(|(at, fact)| (fact.function == "fetch").then_some(at))
            .collect();
        assert_eq!(matched.len(), 2, "two same-named free functions");
        let plain = call_graph(&all, &context);
        assert_eq!(plain.ambiguous(), 1);
        assert!(plain.callees(caller).is_empty());
        let mut overlay = vec![Vec::new(); all.len()];
        overlay[caller] = vec![matched[0]];
        let graph = super::call_graph_with(&all, &context, Some(&overlay));
        assert_eq!(graph.callees(caller), &[matched[0]]);
        assert_eq!(graph.ambiguous(), 0);
        assert_eq!(graph.scip_settled(), 1);
        assert!(graph.settled(caller, matched[0]));
        assert_eq!(
            graph.scip_resolved(),
            0,
            "a settled edge is not recounted as an additive one"
        );
        let mut both = vec![Vec::new(); all.len()];
        both[caller] = matched;
        let refused = super::call_graph_with(&all, &context, Some(&both));
        assert_eq!(refused.ambiguous(), 1);
        assert_eq!(refused.scip_settled(), 0);
    }

    /// A variant of a workspace enum builds a value; nothing is called
    /// there, and the row says so instead of blaming the owner for a
    /// member it never had. A module qualifier is not an owner either:
    /// where the workspace declares no function of that bare name, the
    /// written path names an outside namespace of the same spelling.
    #[test]
    fn a_variant_builds_and_a_module_qualifier_owns_nothing() {
        let text = concat!(
            "pub enum Payload {\n    Opaque,\n}\n",
            "pub struct Held;\nimpl Held { pub fn keep(&self) {} }\n",
            "pub mod io {}\n",
            "pub fn go() {\n    Payload::Opaque();\n    io::persist();\n    Held::missing();\n}\n"
        );
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let all = behavior(&sources);
        let context = super::resolution_context(&sources);
        let (graph, rows) = super::call_graph_rows(&all, &context);
        let count = |wanted: super::RefusalRow| -> usize {
            rows.iter()
                .flatten()
                .filter(|(_, row)| *row == wanted)
                .count()
        };
        assert_eq!(count(super::RefusalRow::Construction), 1, "{rows:?}");
        assert_eq!(graph.constructions(), 1);
        assert_eq!(count(super::RefusalRow::Foreign), 1, "{rows:?}");
        assert_eq!(count(super::RefusalRow::OwnerWithoutMember), 1, "{rows:?}");
        assert_eq!(
            graph.unresolved(),
            graph.external()
                + graph.unknown_receiver()
                + graph.unreachable_named()
                + graph.ambiguous()
                + graph.owned_unmatched()
                + graph.granted()
                + graph.constructions(),
            "a refusal outside every row would be countable only by subtraction"
        );
    }

    /// A member a derive writes is a member the owner carries: the row
    /// says the body is missing from the slice, not that the type lacks
    /// the ability. A trait the type implements grants the same way —
    /// writing `Display` grants `to_string` through its blanket
    /// companion.
    #[test]
    fn a_derived_member_is_granted_rather_than_missing() {
        let text = concat!(
            "#[derive(Clone, Default)]\npub struct Held;\n",
            "pub struct Named;\nimpl std::fmt::Display for Named {\n    fn fmt(&self, out: &mut \
             std::fmt::Formatter<'_>) -> std::fmt::Result { out.write_str(\"x\") }\n}\n",
            "pub struct Bare;\nimpl Bare { pub fn keep(&self) {} }\n",
            "pub fn go() {\n    Held::default();\n    Held::clone(&Held);\n    \
             Named::to_string(&Named);\n    Bare::missing();\n}\n"
        );
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let all = behavior(&sources);
        let context = super::resolution_context(&sources);
        let (graph, rows) = super::call_graph_rows(&all, &context);
        assert_eq!(graph.granted(), 3, "{rows:?}");
        assert_eq!(graph.owned_unmatched(), 1, "{rows:?}");
    }

    /// A question a container answers about itself hands back a truth,
    /// so the call written on that answer has a typed receiver — and
    /// the row is keyed by the owner, so a workspace type carrying the
    /// same member name keeps its own meaning.
    #[test]
    fn a_container_question_types_the_call_written_on_its_answer() {
        let (all, graph) = context_graph_settled(
            "pub struct Held;\nimpl Held { pub fn then(&self) {} }\npub fn go(names: Vec<String>, \
             text: &str) {\n    names.is_empty().then(|| 1);\n    \
             text.starts_with(\"x\").then(|| 2);\n}\n",
            &[
                ("names.is_empty()", "bool"),
                ("text.starts_with(\"x\")", "bool")
            ]
        );
        assert_eq!(graph.unknown_receiver(), 0);
        assert!(
            graph.callees(index_of(&all, "go")).is_empty(),
            "neither answer is the workspace type of that member name"
        );
    }

    /// A pack states what a crate's own field holds and what one step
    /// of a sequence field carries, so a value read off the tree types
    /// the calls written on it.
    #[test]
    fn a_packed_field_types_what_is_read_off_it() {
        let (all, graph) = context_graph(
            "pub struct Held;\nimpl Held { pub fn path(&self) {} }\npub fn go(item: \
             &syn::ItemFn) {\n    for attribute in &item.attrs {\n        \
             attribute.path();\n    }\n    item.sig.ident.to_string();\n}\n"
        );
        assert_eq!(graph.unknown_receiver(), 0);
        assert!(
            graph.callees(index_of(&all, "go")).is_empty(),
            "the attribute is the crate's, not the workspace type of that member name"
        );
    }

    /// A pack states what one step of a crate's own sequence carries,
    /// so the loop binding is typed and every call written on it names
    /// its owner instead of standing untyped.
    #[test]
    fn a_packed_sequence_types_what_its_loop_binds() {
        let (all, graph) = context_graph(
            "use ignore::WalkBuilder;\npub struct Held;\nimpl Held { pub fn path(&self) {} }\npub \
             fn go(root: &str) {\n    let builder = WalkBuilder::new(root);\n    for entry in \
             builder.build() {\n        entry.path();\n    }\n}\n"
        );
        assert_eq!(graph.unknown_receiver(), 0);
        assert!(
            graph.callees(index_of(&all, "go")).is_empty(),
            "the entry is the crate's, not the workspace type of the same member name"
        );
    }

    /// A binding taken from a `match` carries what its arms agree on:
    /// the language requires the agreement, an arm that returns instead
    /// of handing a value back has no type to disagree with, and the
    /// shape this reads — a fallible call matched with its success
    /// bound and its failure returned — is written everywhere.
    #[test]
    fn a_binding_taken_from_a_match_carries_what_the_arms_agree_on() {
        let (all, graph) = context_graph(
            "pub struct Held;\nimpl Held { pub fn keep(&self) {} }\npub fn make() -> Result<Held, \
             u8> { Ok(Held) }\npub fn go() {\n    let held = match make() {\n        Ok(held) => \
             held,\n        Err(_) => return\n    };\n    held.keep();\n}\n"
        );
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "Held::keep"))
        );
    }

    /// A slice has no written path of its own, and a call on one is the
    /// library's, never a workspace method of the same name — naming it
    /// keeps such a receiver out of the untyped row. A path asked for
    /// its parent hands back a path, so the chain written on it stays
    /// typed too.
    #[test]
    fn a_slice_and_a_path_are_named_where_the_language_leaves_them_bare() {
        let (all, graph) = context_graph_settled(
            "use std::path::Path;\npub struct Held;\nimpl Held { pub fn join(&self) {} }\npub fn \
             go(globs: &[String], where_at: &Path) {\n    globs.join(\",\");\n    \
             where_at.parent().unwrap().join(\"x\");\n}\n",
            &[
                ("globs", "&[String]"),
                ("where_at.parent().unwrap()", "&std::path::Path")
            ]
        );
        assert_eq!(graph.unknown_receiver(), 0);
        assert!(
            graph.callees(index_of(&all, "go")).is_empty(),
            "neither call is the workspace method of that name"
        );
    }

    /// A written turbofish on a gathering call states the type it hands
    /// back, so the chain continues from it: the answer is on the page,
    /// and no table needs asking.
    #[test]
    fn a_written_turbofish_types_what_the_gathering_hands_back() {
        let (all, graph) = context_graph(
            "pub struct Held;\nimpl Held { pub fn keep(&self) {} }\npub fn go(counts: \
             Vec<u8>) {\n    counts.iter().collect::<Held>().keep();\n}\n"
        );
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "Held::keep"))
        );
    }

    /// A destructuring `let` over a call handing back a tuple types
    /// every name it binds: which name stands at which position is
    /// written, and the return's own type list answers the rest.
    #[test]
    fn a_destructured_tuple_return_types_every_name_it_binds() {
        let (all, graph) = context_graph(
            "pub struct Graph;\nimpl Graph { pub fn count(&self) {} }\npub struct Rows;\nimpl \
             Rows { pub fn total(&self) {} }\npub fn built() -> (Graph, Rows) { (Graph, Rows) \
             }\npub fn go() {\n    let (graph, rows) = built();\n    graph.count();\n    \
             rows.total();\n}\n"
        );
        let callees = graph.callees(index_of(&all, "go"));
        assert!(callees.contains(&index_of(&all, "Graph::count")));
        assert!(callees.contains(&index_of(&all, "Rows::total")));
    }

    /// An accessor handing back a borrow types the chain written on it:
    /// the language derefs for the call, so `&T` is a `T` to call on,
    /// and reading only the bare form left every such accessor with no
    /// return row at all.
    #[test]
    fn an_accessor_handing_back_a_borrow_types_the_chain() {
        let (all, graph) = context_graph(
            "pub struct Graph;\nimpl Graph { pub fn count(&self) {} }\npub struct \
             Held;\nimpl Held { pub fn graph(&self) -> &Graph { &Graph } }\npub fn go(held: \
             &Held) {\n    held.graph().count();\n}\n"
        );
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "Graph::count"))
        );
    }

    /// A comparison is a truth value by the language's own rule, so a
    /// call written on one has a typed receiver: the workspace method
    /// of the same name is not what it means, and the count says so.
    #[test]
    fn a_call_on_a_comparison_has_a_typed_receiver() {
        let (all, graph) = context_graph(
            "pub struct Held;\nimpl Held { pub fn then(&self) {} }\npub fn go(count: usize) {\n    \
             (count > 1).then(|| count);\n    (!count.is_power_of_two()).then_some(count);\n}\n"
        );
        assert_eq!(
            graph.unknown_receiver(),
            0,
            "both receivers are written truths"
        );
        assert!(graph.callees(index_of(&all, "go")).is_empty());
    }

    /// A pair destructured from iterating a map is typed on both sides,
    /// and the reading follows the adapters that hand the element on
    /// unchanged — a written `take` between the iteration and the loop
    /// does not blind it.
    #[test]
    fn a_map_pair_types_both_sides_through_a_passing_adapter() {
        let (all, graph) = context_graph(
            "use std::collections::BTreeMap;\npub struct Row;\nimpl Row { pub fn render(&self) \
             {} }\npub fn go() {\n    let held: BTreeMap<String, Row> = BTreeMap::new();\n    \
             for (_name, row) in held.iter().take(3) {\n        row.render();\n    }\n}\n"
        );
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "Row::render")),
            "the value side of the pair carries the type that answers the call"
        );
    }

    /// A file no crate root reaches is compiled by nothing: a fixture
    /// beside a rule, a loose script. A compiled caller cannot bind
    /// into it, however well the name matches — but a caller that is
    /// itself loose still can, so reading a bare subtree keeps every
    /// candidate it had.
    #[test]
    fn a_compiled_caller_never_binds_into_an_uncompiled_file() {
        let fixture = Source::new(
            PathBuf::from("crates/rules/fixtures/one.rs"),
            "pub struct Request;\nimpl Request { pub fn new() -> Self { Self } }\n".to_owned()
        );
        let sources = [
            Source::new(
                PathBuf::from("crates/one/src/lib.rs"),
                "pub fn go() { Request::new(); }\n".to_owned()
            ),
            fixture.clone()
        ];
        let all = behavior(&sources);
        let settled = mounted(&[("one", "crates/one/src/lib.rs")]);
        let (graph, rows) = super::call_graph_rows(
            &all,
            &super::resolution_context_with(&sources, Some(&settled))
        );
        assert!(graph.callees(index_of(&all, "go")).is_empty());
        assert_eq!(
            graph.unreachable_named(),
            1,
            "the name is written, and written where no build holds it: {rows:?}"
        );
        assert_eq!(graph.owned_unmatched(), 0, "{rows:?}");
        let loose = [
            Source::new(
                PathBuf::from("crates/rules/fixtures/two.rs"),
                "pub fn go() { Request::new(); }\n".to_owned()
            ),
            fixture
        ];
        let held = behavior(&loose);
        let reading = call_graph(&held, &super::resolution_context(&loose));
        assert!(
            reading
                .callees(index_of(&held, "go"))
                .contains(&index_of(&held, "Request::new"))
        );
    }

    /// A crate-private method of a neighbouring crate is a coincidence
    /// of spelling, not a candidate: another crate cannot name it at
    /// all, so a call on an untyped receiver carrying that name is
    /// outside code rather than the engine's own typing gap. A
    /// trait-impl method is exempt — the trait's reach governs it.
    #[test]
    fn a_private_method_of_another_crate_is_not_a_candidate() {
        let calling = Source::new(
            PathBuf::from("crates/one/src/lib.rs"),
            "pub fn go() { opaque().join(\"x\"); }\n".to_owned()
        );
        let private = Source::new(
            PathBuf::from("crates/two/src/lib.rs"),
            "pub struct Held;\nimpl Held { fn join(&self) {} }\n".to_owned()
        );
        let sources = [calling.clone(), private];
        let all = behavior(&sources);
        let graph = call_graph(&all, &super::resolution_context(&sources));
        assert_eq!(graph.unknown_receiver(), 0, "the neighbour cannot be named");
        assert_eq!(graph.unreachable_named(), 1, "named, linkable from nowhere");
        let shared = Source::new(
            PathBuf::from("crates/two/src/lib.rs"),
            "pub struct Held;\nimpl Held { pub fn join(&self) {} }\n".to_owned()
        );
        let open = [calling, shared];
        let seen = behavior(&open);
        let reached = call_graph(&seen, &super::resolution_context(&open));
        assert_eq!(
            reached.unreachable_named(),
            0,
            "a public neighbour stays a candidate"
        );
        assert!(
            reached
                .callees(index_of(&seen, "go"))
                .contains(&index_of(&seen, "Held::join")),
            "and the sole public match binds"
        );
    }

    /// An unqualified call means the definition standing in the calling
    /// scope, even where the workspace carries the name several times:
    /// Rust resolves an unqualified path in the calling scope first, so
    /// this is the language's own answer rather than a pick between
    /// siblings. A namesake one module deeper is a different scope and
    /// does not answer the call — the file is not the granularity.
    #[test]
    fn an_unqualified_call_means_the_definition_in_the_calling_scope() {
        let home = "pub fn ping() {}\npub fn caller() { ping(); }\n";
        let away = Source::new(
            PathBuf::from("crates/two/src/lib.rs"),
            "pub fn ping() {}\n".to_owned()
        );
        let sources = [
            Source::new(PathBuf::from("crates/one/src/lib.rs"), home.to_owned()),
            away.clone()
        ];
        let all = behavior(&sources);
        let graph = call_graph(&all, &super::resolution_context(&sources));
        let caller = index_of(&all, "caller");
        let local = all
            .iter()
            .position(|fact| fact.function == "ping" && fact.source == 0)
            .expect("the local definition");
        assert_eq!(graph.callees(caller), &[local]);
        assert_eq!(graph.ambiguous(), 0);
        let deeper = concat!(
            "pub fn caller() { ping(); }\n",
            "mod inner { pub fn ping() {} }\n"
        );
        let guarded = [
            Source::new(PathBuf::from("crates/one/src/lib.rs"), deeper.to_owned()),
            away
        ];
        let held = behavior(&guarded);
        let refused = call_graph(&held, &super::resolution_context(&guarded));
        assert!(refused.callees(index_of(&held, "caller")).is_empty());
        assert_eq!(refused.ambiguous(), 1);
    }

    /// The same settlement reaches the untyped receiver: a `?::` name
    /// several methods carry is one confirmed match away from binding,
    /// and the row it would have landed in empties.
    #[test]
    fn the_index_settles_an_untyped_receiver_to_its_one_confirmed_method() {
        let all = facts(
            "fn go() { opaque().ping(); }\nstruct A;\nimpl A { fn ping(&self) {} }\nstruct \
             B;\nimpl B { fn ping(&self) {} }"
        );
        let context = super::ResolutionContext::default();
        let caller = index_of(&all, "go");
        let chosen = index_of(&all, "A::ping");
        let plain = call_graph(&all, &context);
        assert_eq!(plain.unknown_receiver(), 1);
        let mut overlay = vec![Vec::new(); all.len()];
        overlay[caller] = vec![chosen];
        let graph = super::call_graph_with(&all, &context, Some(&overlay));
        assert!(graph.callees(caller).contains(&chosen));
        assert_eq!(graph.unknown_receiver(), 0);
        assert_eq!(graph.scip_settled(), 1);
    }

    #[test]
    fn trait_object_calls_dispatch_to_every_implementor() {
        let (all, graph) = context_graph(
            "trait Analyzer { fn analyze(&self); }\nstruct A;\nimpl Analyzer for A { fn \
             analyze(&self) {} }\nstruct B;\nimpl Analyzer for B { fn analyze(&self) {} \
             }\nstruct Hub { analyzers: Vec<Box<dyn Analyzer>> }\nimpl Hub { fn run(&self) { \
             for analyzer in &self.analyzers { analyzer.analyze(); } } }"
        );
        let callees = graph.callees(index_of(&all, "Hub::run"));
        assert!(callees.contains(&index_of(&all, "A::analyze")));
        assert!(callees.contains(&index_of(&all, "B::analyze")));
    }

    #[test]
    fn dyn_reference_parameters_dispatch_too() {
        let (all, graph) = context_graph(
            "trait Sink { fn write(&self); }\nstruct File;\nimpl Sink for File { fn write(&self) \
             {} }\nstruct Net;\nimpl Sink for Net { fn write(&self) {} }\nfn drain(sink: &dyn \
             Sink) { sink.write(); }"
        );
        let callees = graph.callees(index_of(&all, "drain"));
        assert!(callees.contains(&index_of(&all, "File::write")));
        assert!(callees.contains(&index_of(&all, "Net::write")));
    }

    #[test]
    fn free_function_calls_resolve_by_bare_name() {
        let all = facts("fn helper() {}\nfn caller() { helper(); }");
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "caller")),
            &[index_of(&all, "helper")]
        );
        assert_eq!(graph.resolved(), 1);
    }

    #[test]
    fn field_receivers_resolve_through_struct_definitions() {
        let all = facts(
            "struct Engine;\nimpl Engine { fn start(&self) {} }\nstruct Car { engine: Engine \
             }\nimpl Car { fn drive(&self) { self.engine.start(); } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "Car::drive")),
            &[index_of(&all, "Engine::start")]
        );
    }

    #[test]
    fn loop_variables_type_through_collection_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) { for item in items { item.ping(); } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn closure_parameters_type_through_iterated_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: Vec<S>) { items.iter().for_each(|item| item.ping()); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn indexed_receivers_type_through_collection_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) { items[0].ping(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn field_collections_type_their_iterated_elements() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nstruct Holder { items: Vec<S> }\nimpl Holder { fn go(&self) { for item in \
             &self.items { item.ping(); } } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "Holder::go")),
            &[index_of(&all, "S::ping")]
        );
    }

    #[test]
    fn unwrapped_external_constructors_attribute_calls_outside() {
        let all = facts(
            "struct S;\nimpl S { fn path(&self) {} }\nstruct T;\nimpl T { fn path(&self) {} \
             }\nfn go() { let dir = TempDir::new().unwrap(); dir.path(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.unknown_receiver(), 0);
        assert_eq!(graph.external(), 3);
    }

    #[test]
    fn builder_chains_type_through_return_tables() {
        let all = facts(
            "struct R;\nimpl R { fn new() -> Self { R } fn with_color(self) -> Self { self } fn \
             render(&self) {} }\nstruct Q;\nimpl Q { fn render(&self) {} }\nfn go() { let out = \
             R::new().with_color(); out.render(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "R::render"))
        );
    }

    #[test]
    fn enumerate_tuples_bind_the_element_half() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) { for (index, item) in items.iter().enumerate() { \
             item.ping(); } }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "S::ping"))
        );
    }

    #[test]
    fn optional_accessors_yield_the_element_type() {
        let all = facts(
            "struct S;\nimpl S { fn ping(&self) {} }\nstruct T;\nimpl T { fn ping(&self) {} \
             }\nfn go(items: &[S]) -> Option<()> { items.get(0)?.ping(); Some(()) }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(
            graph
                .callees(index_of(&all, "go"))
                .contains(&index_of(&all, "S::ping"))
        );
    }

    #[test]
    fn known_external_constructors_attribute_calls_outside() {
        let all = facts(
            "struct S;\nimpl S { fn path(&self) {} }\nstruct T;\nimpl T { fn path(&self) {} \
             }\nfn go() { let dir = tempfile::tempdir().unwrap(); dir.path(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.unknown_receiver(), 0);
    }

    #[test]
    fn unmatched_candidates_split_into_external_and_unknown_receiver() {
        let all = facts(
            "fn go() { let text = String::from(\"x\"); opaque().push(text); opaque().ping(); \
             }\nstruct A;\nimpl A { fn ping(&self) {} }\nstruct B;\nimpl B { fn ping(&self) {} \
             }\nfn miss() { A::gone(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.resolved(), 0);
        assert_eq!(graph.external(), 3);
        assert_eq!(graph.unknown_receiver(), 1);
        assert_eq!(graph.unresolved(), 5);
    }

    #[test]
    fn associated_calls_resolve_by_qualified_name() {
        let all =
            facts("struct Board;\nimpl Board { fn reset() {} }\nfn wipe() { Board::reset(); }");
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "wipe")),
            &[index_of(&all, "Board::reset")]
        );
    }

    #[test]
    fn method_calls_resolve_through_typed_receivers() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn balance(&self) -> i64 { \
             self.balance } }\nfn audit(wallet: &Wallet) -> i64 { wallet.balance() }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(
            graph.callees(index_of(&all, "audit")),
            &[index_of(&all, "Wallet::balance")]
        );
    }

    #[test]
    fn ambiguous_bare_names_stay_unresolved() {
        let all = facts(
            "struct A;\nimpl A { fn go(&self) {} }\nstruct B;\nimpl B { fn go(&self) {} }\nfn \
             run(x: u32) { x.go(); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert!(graph.callees(index_of(&all, "run")).is_empty());
        assert_eq!(graph.unresolved(), 1);
    }

    #[test]
    fn effects_propagate_transitively() {
        let all = facts(
            "fn log(text: &str) { println!(\"{text}\"); }\nfn middle(text: &str) { log(text); \
             }\nfn top(text: &str) { middle(text); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "top")], vec![Effect::ConsoleIo]);
        assert_eq!(effects[index_of(&all, "middle")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn recursion_reaches_a_fixpoint() {
        let all = facts(
            "fn ping(n: u32) { if n > 0 { pong(n - 1); } }\nfn pong(n: u32) { println!(\"{n}\"); \
             ping(n); }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        let effects = transitive_effects(&all, &graph);
        assert_eq!(effects[index_of(&all, "ping")], vec![Effect::ConsoleIo]);
    }

    #[test]
    fn external_calls_are_honestly_unresolved() {
        let all = facts("fn read() -> String { std::fs::read_to_string(\"x\").unwrap() }");
        let graph = call_graph(&all, &super::ResolutionContext::default());
        assert_eq!(graph.resolved(), 0);
        assert!(graph.unresolved() > 0);
    }

    /// Every refused candidate lands in exactly one named row: a
    /// remainder derivable only by subtraction is the hole the
    /// accounting exists to close.
    #[test]
    fn every_refusal_lands_in_exactly_one_named_row() {
        let sources = [
            Source::new(
                PathBuf::from("crates/one/src/lib.rs"),
                "mod other;\npub struct Wallet;\nimpl Wallet { pub fn credit(&self) {} }\npub \
                 fn caller(w: Wallet) {\n    ping();\n    Wallet::missing();\n    \
                 std::fs::read_to_string(\"x\").ok();\n}"
                    .to_owned()
            ),
            Source::new(
                PathBuf::from("crates/one/src/other.rs"),
                "pub fn ping() {}".to_owned()
            ),
            Source::new(
                PathBuf::from("crates/two/src/lib.rs"),
                "pub fn ping() {}".to_owned()
            )
        ];
        let all = crate::semantic::behavior(&sources);
        let graph = call_graph(&all, &super::resolution_context(&sources));
        assert_eq!(
            graph.ambiguous(),
            1,
            "two linkable ping definitions, neither in the caller's own file"
        );
        assert_eq!(graph.owned_unmatched(), 1, "an owner without the member");
        assert!(graph.external() >= 1, "the std call is foreign");
        assert_eq!(
            graph.unresolved(),
            graph.external()
                + graph.unknown_receiver()
                + graph.unreachable_named()
                + graph.ambiguous()
                + graph.owned_unmatched(),
            "a refusal outside every row would be countable only by subtraction"
        );
    }

    /// A name defined only where the caller cannot link is its own row:
    /// external by elimination is a different unknowing than a name the
    /// workspace never declares.
    #[test]
    fn a_name_defined_only_in_an_unlinkable_target_is_its_own_row() {
        let sources = [
            Source::new(
                PathBuf::from("crates/demo/tests/alpha.rs"),
                "struct Client;\nimpl Client { fn fetch(&self) {} }\nfn drive(client: Client) { \
                 client.fetch(); }"
                    .to_owned()
            ),
            Source::new(
                PathBuf::from("crates/demo/tests/beta.rs"),
                "fn poke() { helper().fetch(); }".to_owned()
            )
        ];
        let all = crate::semantic::behavior(&sources);
        let graph = call_graph(&all, &super::resolution_context(&sources));
        assert_eq!(
            graph.unreachable_named(),
            1,
            "the other test crate's method exists, but not for this caller"
        );
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
        let settled = mounted(&[
            ("rusty_core", "crates/rusty-core/src/lib.rs"),
            ("rusty_app", "crates/rusty-app/src/lib.rs")
        ]);
        let all = crate::semantic::behavior(&sources);
        let graph = call_graph(
            &all,
            &super::resolution_context_with(&sources, Some(&settled))
        );
        assert_eq!(
            graph.callees(index_of(&all, "run")),
            &[index_of(&all, "helper")]
        );
    }

    #[test]
    fn factory_typed_receivers_resolve_method_calls() {
        let all = facts(
            "struct Wallet { balance: i64 }\nimpl Wallet { fn balance(&self) -> i64 { \
             self.balance } }\nfn make_wallet() -> Wallet { Wallet { balance: 0 } }\nfn \
             audit() -> i64 { let w = make_wallet(); w.balance() }\nfn peek() -> i64 { \
             make_wallet().balance() }"
        );
        let graph = call_graph(&all, &super::ResolutionContext::default());
        let target = index_of(&all, "Wallet::balance");
        assert!(graph.callees(index_of(&all, "audit")).contains(&target));
        assert!(graph.callees(index_of(&all, "peek")).contains(&target));
    }
}
