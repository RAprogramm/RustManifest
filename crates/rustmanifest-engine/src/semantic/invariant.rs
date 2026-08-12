// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! User-declared behavioral invariants enforced over extracted facts.
//!
//! An invariant constrains what the behavioral tier observed: who may
//! write a field (`field` + `write-only-from`), or where a side-effect
//! category is banned (`effect` + `deny-in`). Enforcement joins the
//! [`behavior`] facts with the module graph — each function's file maps to
//! its module path, module lists match by `::`-aligned prefix — and every
//! violation carries the full derivation as a trace.
//!
//! Effect bans are enforced **transitively**: a function in a denied
//! module violates the ban even when the effect happens inside a callee,
//! not its own body. The trace then carries the resolved call chain from
//! the violating function down to the function that performs the effect
//! directly.
//!
//! The chain is as wide as the call graph handed to the check. With a
//! SCIP index supplied, [`check_invariants_with`] enforces the same
//! invariants over the name-resolved graph, so a ban also fires through
//! calls only the index resolves — macro-generated bodies, trait
//! dispatch, receivers the syntactic tier cannot type. Every trace step
//! names the provenance of the edge it walks, so an index-owed
//! derivation is never presented as a syntactic one.

pub(in crate::semantic) mod chain;
mod effect_ban;
mod field_write;
mod layer_boundary;
mod scope;

use rustmanifest_schema::{Finding, Invariant, Location, Severity};

use self::{
    effect_ban::effect_violation, field_write::field_violation, layer_boundary::call_violation
};
use super::{
    calls::{call_graph_with, transitive_effects},
    module_graph::ModuleGraph,
    module_overview,
    scip::ScipEdges
};
use crate::{pragma, source::Source};

/// Checks every invariant over `sources` using the syntactic call graph.
///
/// Findings carry the invariant's `id` as their rule id, or a positional
/// `RM-INV-NNN` when the entry declares none; severity defaults to
/// [`Severity::Error`].
#[must_use]
pub fn check_invariants(sources: &[Source], invariants: &[Invariant]) -> Vec<Finding> {
    check_invariants_with(sources, invariants, None)
}

/// Checks every invariant over `sources`, optionally over the call graph
/// a SCIP index resolves, honoring pragma suppression in the file each
/// finding anchors to.
///
/// The overlay is purely additive: it can only widen the reachability a
/// ban is judged against, never suppress a finding the syntactic graph
/// already produced. Passing `None` is exactly [`check_invariants`].
#[must_use]
pub fn check_invariants_with(
    sources: &[Source],
    invariants: &[Invariant],
    scip: Option<&ScipEdges>
) -> Vec<Finding> {
    check_invariants_settled(sources, invariants, scip, None)
}

/// Checks every invariant over `sources`, reading which module each file
/// belongs to from the tree a resolver settled.
///
/// A module-scoped ban is a statement about modules, so without a settled
/// tree there are no modules to scope it to and the check answers nothing.
#[must_use]
pub fn check_invariants_settled(
    sources: &[Source],
    invariants: &[Invariant],
    scip: Option<&ScipEdges>,
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<Finding> {
    if invariants.is_empty() {
        return Vec::new();
    }
    let facts = crate::semantic::behavior::behavior_with(sources, settled);
    let overlay = scip.map(|edges| edges.candidates(&facts, sources));
    let graph = call_graph_with(
        &facts,
        &super::calls::resolution_context_with(sources, settled),
        overlay.as_deref()
    );
    let transitive = transitive_effects(&facts, &graph);
    check_invariants_over(sources, &facts, &graph, &transitive, invariants, settled)
}

/// Checks every invariant over facts and graphs already derived, so a
/// caching caller pays for none of them twice.
///
/// `transitive` must be the transitive effects of `facts` closed over
/// `graph` — the same triple one build of a snapshot holds together.
#[must_use]
pub fn check_invariants_over(
    sources: &[Source],
    facts: &[super::behavior::FunctionFacts],
    graph: &super::calls::CallGraph,
    transitive: &[Vec<super::behavior::Effect>],
    invariants: &[Invariant],
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<Finding> {
    if invariants.is_empty() {
        return Vec::new();
    }
    let overview = module_overview::of(&ModuleGraph::of(sources, settled), sources);
    let module_of = |source: usize| -> String {
        sources
            .get(source)
            .and_then(|entry| {
                overview
                    .modules()
                    .iter()
                    .find(|module| module.file == entry.path())
            })
            .map_or_else(|| "?".to_owned(), |module| module.path.clone())
    };
    let mut findings = Vec::new();
    for (index, invariant) in invariants.iter().enumerate() {
        let rule_id = invariant
            .id
            .clone()
            .unwrap_or_else(|| format!("RM-INV-{:03}", index + 1));
        let severity = invariant.severity.unwrap_or(Severity::Error);
        for (fact_index, fact) in facts.iter().enumerate() {
            let Some(reached) = transitive.get(fact_index) else {
                continue;
            };
            let module = module_of(fact.source);
            let violation = field_violation(invariant, fact, &module)
                .or_else(|| {
                    effect_violation(invariant, facts, fact_index, reached, graph, &module)
                })
                .or_else(|| {
                    call_violation(invariant, facts, fact_index, graph, &module, &module_of)
                });
            let Some((detail, trace)) = violation else {
                continue;
            };
            let Some(source) = sources.get(fact.source) else {
                continue;
            };
            let text = source.text();
            let pragmas = source.pragmas();
            let start = fact.range.start as usize;
            if let Some(active) = pragma::suppressing(text, start, &rule_id, pragmas) {
                source.record_pragma_use(active.line_index);
                continue;
            }
            findings.push(Finding {
                rule_id: rule_id.clone(),
                severity,
                location: Location {
                    file:  source.path().to_path_buf(),
                    range: fact.range
                },
                message: detail,
                rationale_uri: format!("rustmanifest://invariants/{rule_id}"),
                trace,
                fixes: Vec::new()
            });
        }
    }
    findings
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

    use rustmanifest_schema::{Finding, Invariant, Severity};

    use super::check_invariants_settled;
    use crate::{semantic::module_graph::declared_tree, source::Source};

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    /// The check with the module tree the fixture's layout implies.
    fn checked(sources: &[Source], invariants: &[Invariant]) -> Vec<Finding> {
        let settled = declared_tree(sources);
        check_invariants_settled(sources, invariants, None, Some(&settled))
    }

    fn wallet_sources() -> Vec<Source> {
        vec![
            source(
                "src/lib.rs",
                "mod ledger; mod api;\npub struct Wallet { pub balance: i64 }"
            ),
            source(
                "src/ledger.rs",
                "use crate::Wallet;\npub fn post(wallet: &mut Wallet, amount: i64) { \
                 wallet.balance += amount; }"
            ),
            source(
                "src/api.rs",
                "use crate::Wallet;\npub fn hack(wallet: &mut Wallet) { wallet.balance = 0; }"
            ),
        ]
    }

    fn balance_invariant() -> Invariant {
        Invariant {
            id:              Some("WALLET-BALANCE".to_owned()),
            field:           Some("Wallet.balance".to_owned()),
            write_only_from: vec!["crate::ledger".to_owned()],
            effect:          None,
            deny_in:         Vec::new(),
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        }
    }

    /// A caller holding a snapshot's derivations judges the same
    /// violation as the fresh check, paying for none of them twice.
    #[test]
    fn a_precomputed_snapshot_judges_the_same_violation() {
        let sources = wallet_sources();
        let settled = declared_tree(&sources);
        let facts = crate::semantic::behavior::behavior_with(&sources, Some(&settled));
        let graph = crate::semantic::calls::call_graph(
            &facts,
            &crate::semantic::calls::resolution_context_with(&sources, Some(&settled))
        );
        let transitive = crate::semantic::calls::transitive_effects(&facts, &graph);
        let held = super::check_invariants_over(
            &sources,
            &facts,
            &graph,
            &transitive,
            &[balance_invariant()],
            Some(&settled)
        );
        assert_eq!(held, checked(&sources, &[balance_invariant()]));
        assert_eq!(held.len(), 1, "{held:?}");
    }

    #[test]
    fn writer_outside_the_allowlist_is_reported_with_a_trace() {
        let findings = checked(&wallet_sources(), &[balance_invariant()]);
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.rule_id, "WALLET-BALANCE");
        assert_eq!(finding.severity, Severity::Error);
        assert!(finding.message.contains("`hack` writes `Wallet.balance`"));
        assert!(finding.message.contains("crate::api"));
        assert_eq!(finding.trace.len(), 3);
        assert!(finding.trace[1].fact.contains("crate::api"));
    }

    #[test]
    fn allowed_module_and_its_children_pass() {
        let sources = vec![
            source(
                "src/lib.rs",
                "mod ledger;\npub struct Wallet { pub balance: i64 }"
            ),
            source("src/ledger.rs", "mod posting;"),
            source(
                "src/ledger/posting.rs",
                "use crate::Wallet;\npub fn post(wallet: &mut Wallet) { wallet.balance += 1; }"
            ),
        ];
        assert!(checked(&sources, &[balance_invariant()]).is_empty());
    }

    /// The unproven-kind database grade is bannable by its recorded
    /// name: a runtime-built statement in a denied module violates the
    /// ban exactly as a written one would.
    #[test]
    fn a_db_ban_catches_a_runtime_statement() {
        let sources = vec![
            source("src/lib.rs", "mod api;"),
            source(
                "src/api.rs",
                "pub async fn purge(pool: &sqlx::PgPool, sql: &str) { sqlx::query(sql); }"
            ),
        ];
        let ban = Invariant {
            id:              Some("NO-DB".to_owned()),
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("db".to_owned()),
            deny_in:         vec!["crate::api".to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        };
        let findings = checked(&sources, &[ban]);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].message.contains("purge"), "{findings:?}");
    }

    /// The category name covers its kinds: one `db` declaration denies
    /// a kind-proven read exactly as it denies the unproven grade.
    #[test]
    fn a_db_ban_covers_a_kind_proven_statement() {
        let sources = vec![
            source("src/lib.rs", "mod api;"),
            source(
                "src/api.rs",
                "pub async fn list(pool: &sqlx::PgPool) { sqlx::query(\"SELECT id FROM tasks\");                  }"
            ),
        ];
        let ban = Invariant {
            id:              Some("NO-DB".to_owned()),
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("db".to_owned()),
            deny_in:         vec!["crate::api".to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        };
        let findings = checked(&sources, &[ban]);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].message.contains("list"), "{findings:?}");
    }

    fn net_ban(module: &str) -> Invariant {
        Invariant {
            id:              Some("NO-NET".to_owned()),
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("net".to_owned()),
            deny_in:         vec![module.to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        }
    }

    #[test]
    fn effect_ban_sees_through_wrapper_closures() {
        let sources = vec![
            source("src/lib.rs", "mod domain;"),
            source(
                "src/domain.rs",
                "pub struct Src { inner: IoSource<TcpStream> }\nimpl Src { pub fn pump(&self) { \
                 self.inner.do_io(|stream| { stream.read(&mut []).ok(); }); } }"
            ),
        ];
        let findings = checked(&sources, &[net_ban("crate::domain")]);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("pump"));
    }

    #[test]
    fn effect_ban_sees_through_await_edges() {
        let sources = vec![
            source("src/lib.rs", "mod domain; mod jobs;"),
            source(
                "src/jobs.rs",
                "pub struct Job;\npub fn produce() -> Job { Job }\nimpl Job { pub fn poll(&self) \
                 -> u8 { std::net::TcpStream::connect(\"h\").ok(); 0 } }"
            ),
            source(
                "src/domain.rs",
                "use crate::jobs::produce;\npub async fn drive() { produce().await; }"
            ),
        ];
        let findings = checked(&sources, &[net_ban("crate::domain")]);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("drive"));
    }

    #[test]
    fn effect_ban_sees_through_extension_defaults() {
        let sources = vec![
            source("src/lib.rs", "mod domain; mod io;"),
            source(
                "src/io.rs",
                "pub trait Base { fn send(&self); }\npub struct Conn;\nimpl Base for Conn { fn \
                 send(&self) { std::net::TcpStream::connect(\"h\").ok(); } }\npub trait Ext: \
                 Base {\n    fn send_all(&self) { self.send(); }\n}"
            ),
            source(
                "src/domain.rs",
                "use crate::io::Conn;\npub fn go(conn: Conn) { conn.send_all(); }"
            ),
        ];
        let findings = checked(&sources, &[net_ban("crate::domain")]);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("go"));
    }

    #[test]
    fn ambiguous_dispatch_fires_the_ban_and_admits_the_possibility() {
        let sources = vec![
            source("src/lib.rs", "mod domain; mod sinks;"),
            source(
                "src/sinks.rs",
                "pub trait Sink { fn flush(&self); }\npub struct Quiet;\nimpl Sink for Quiet { \
                 fn flush(&self) {} }\npub struct Wire;\nimpl Sink for Wire { fn flush(&self) { \
                 std::net::TcpStream::connect(\"h\").ok(); } }"
            ),
            source(
                "src/domain.rs",
                "use crate::sinks::Sink;\npub fn drain(sink: &dyn Sink) { sink.flush(); }"
            ),
        ];
        let findings = checked(&sources, &[net_ban("crate::domain")]);
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0]
                .trace
                .iter()
                .any(|step| step.reason.contains("possible, not certain"))
        );
    }

    #[test]
    fn effect_ban_reports_the_effectful_function() {
        let sources = vec![
            source("src/lib.rs", "mod domain;"),
            source(
                "src/domain.rs",
                "pub fn compute() -> u64 { std::fs::read(\"x\").map(|b| b.len() as \
                 u64).unwrap_or(0) }"
            ),
        ];
        let invariant = Invariant {
            id:              None,
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("fs".to_owned()),
            deny_in:         vec!["crate::domain".to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        Some(Severity::Warning)
        };
        let findings = checked(&sources, &[invariant]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "RM-INV-001");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].message.contains("performs `fs`"));
    }

    #[test]
    fn pragma_suppresses_an_invariant_finding() {
        let sources = vec![
            source(
                "src/lib.rs",
                "mod api;\npub struct Wallet { pub balance: i64 }"
            ),
            source(
                "src/api.rs",
                "use crate::Wallet;\n// rustmanifest: allow(WALLET-BALANCE) \
                 reason=\"migration\"\npub fn hack(wallet: &mut Wallet) { wallet.balance = 0; }"
            ),
        ];
        assert!(checked(&sources, &[balance_invariant()]).is_empty());
    }

    #[test]
    fn no_invariants_means_no_work() {
        assert!(checked(&wallet_sources(), &[]).is_empty());
    }

    #[test]
    fn transitive_effect_in_a_denied_module_is_reported_with_the_chain() {
        let sources = vec![
            source("src/lib.rs", "mod domain; mod io;"),
            source(
                "src/domain.rs",
                "pub fn decide(flag: bool) { if flag { crate::io::persist(); } }"
            ),
            source(
                "src/io.rs",
                "pub fn persist() { std::fs::write(\"x\", \"y\").ok(); }"
            ),
        ];
        let invariant = Invariant {
            id:              Some("NO-FS-IN-DOMAIN".to_owned()),
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("fs".to_owned()),
            deny_in:         vec!["crate::domain".to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        };
        let findings = checked(&sources, &[invariant]);
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert!(
            finding
                .message
                .contains("`decide` transitively performs `fs`")
        );
        assert!(finding.message.contains("through `persist`"));
        assert!(
            finding
                .trace
                .iter()
                .any(|step| step.fact.contains("`decide` calls fn `persist`"))
        );
        assert!(
            finding
                .trace
                .iter()
                .any(|step| step.fact.contains("`persist` performs `fs`"))
        );
    }

    #[test]
    fn transitive_effect_outside_denied_modules_passes() {
        let sources = vec![
            source("src/lib.rs", "mod app; mod io;"),
            source(
                "src/app.rs",
                "pub fn run(flag: bool) { if flag { crate::io::persist(); } }"
            ),
            source(
                "src/io.rs",
                "pub fn persist() { std::fs::write(\"x\", \"y\").ok(); }"
            ),
        ];
        let invariant = Invariant {
            id:              None,
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("fs".to_owned()),
            deny_in:         vec!["crate::domain".to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        };
        assert!(checked(&sources, &[invariant]).is_empty());
    }

    #[test]
    fn layer_boundary_reports_the_reach_chain() {
        let sources = vec![
            source("src/lib.rs", "mod api; mod domain; mod storage;"),
            source("src/api.rs", "pub fn handle() { crate::domain::decide(); }"),
            source(
                "src/domain.rs",
                "pub fn decide() { crate::storage::save(); }"
            ),
            source("src/storage.rs", "pub fn save() {}"),
        ];
        let invariant = Invariant {
            id:              Some("API-NOT-STORAGE".to_owned()),
            field:           None,
            write_only_from: Vec::new(),
            effect:          None,
            deny_in:         Vec::new(),
            call_deny_from:  Some("crate::api".to_owned()),
            call_deny_to:    Some("crate::storage".to_owned()),
            severity:        None
        };
        let findings = checked(&sources, &[invariant]);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("transitively reaches"));
        assert!(
            findings[0]
                .trace
                .iter()
                .any(|step| step.fact.contains("`handle` calls fn `decide`"))
        );
    }

    #[test]
    fn layers_that_never_touch_pass() {
        let sources = vec![
            source("src/lib.rs", "mod api; mod storage;"),
            source("src/api.rs", "pub fn handle() {}"),
            source("src/storage.rs", "pub fn save() {}"),
        ];
        let invariant = Invariant {
            id:              None,
            field:           None,
            write_only_from: Vec::new(),
            effect:          None,
            deny_in:         Vec::new(),
            call_deny_from:  Some("crate::api".to_owned()),
            call_deny_to:    Some("crate::storage".to_owned()),
            severity:        None
        };
        assert!(checked(&sources, &[invariant]).is_empty());
    }
}
