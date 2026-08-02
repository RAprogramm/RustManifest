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

use rustmanifest_schema::{Finding, Invariant, Location, Severity, TraceStep};

use super::{
    behavior::{FunctionFacts, behavior},
    calls::{CallGraph, call_graph_with, transitive_effects},
    module_overview::module_overview,
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
    if invariants.is_empty() {
        return Vec::new();
    }
    let facts = behavior(sources);
    let overview = module_overview(sources);
    let overlay = scip.map(|edges| edges.candidates(&facts, sources));
    let graph = call_graph_with(
        &facts,
        &super::calls::resolution_context(sources),
        overlay.as_deref()
    );
    let transitive = transitive_effects(&facts, &graph);
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
            let module = module_of(fact.source);
            let violation = field_violation(invariant, fact, &module)
                .or_else(|| {
                    effect_violation(
                        invariant,
                        &facts,
                        fact_index,
                        &transitive[fact_index],
                        &graph,
                        &module
                    )
                })
                .or_else(|| {
                    call_violation(invariant, &facts, fact_index, &graph, &module, &module_of)
                });
            let Some((detail, trace)) = violation else {
                continue;
            };
            let Some(source) = sources.get(fact.source) else {
                continue;
            };
            let text = source.text();
            let pragmas = pragma::collect(text);
            let start = fact.range.start as usize;
            if let Some(active) = pragma::suppressing(text, start, &rule_id, &pragmas) {
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

/// Checks the field-writer constraint of one invariant against one
/// function's facts.
fn field_violation(
    invariant: &Invariant,
    fact: &super::behavior::FunctionFacts,
    module: &str
) -> Option<(String, Vec<TraceStep>)> {
    let field = invariant.field.as_deref()?;
    if !fact.writes.iter().any(|written| written == field) {
        return None;
    }
    if covered_by_any(module, &invariant.write_only_from) {
        return None;
    }
    let allowed = invariant.write_only_from.join(", ");
    let detail = format!(
        "invariant violated: `{}` writes `{field}` from module `{module}`, but only [{allowed}] \
         may write it",
        fact.function
    );
    let trace = vec![
        TraceStep {
            fact:   format!("fn `{}` writes `{field}`", fact.function),
            reason: "observed assignment or mutating call in the body".to_owned()
        },
        TraceStep {
            fact:   format!("fn `{}` lives in module `{module}`", fact.function),
            reason: "module graph maps its file to this path".to_owned()
        },
        TraceStep {
            fact:   format!("allowed writers: [{allowed}]"),
            reason: "module is not covered by any allowed prefix".to_owned()
        },
    ];
    Some((detail, trace))
}

/// Checks the effect-ban constraint of one invariant against one
/// function's direct and transitive effects.
fn effect_violation(
    invariant: &Invariant,
    facts: &[FunctionFacts],
    fact_index: usize,
    reachable: &[super::behavior::Effect],
    graph: &CallGraph,
    module: &str
) -> Option<(String, Vec<TraceStep>)> {
    let effect = invariant.effect.as_deref()?;
    let fact = facts.get(fact_index)?;
    if !reachable
        .iter()
        .any(|observed| observed.to_string() == effect)
    {
        return None;
    }
    if !covered_by_any(module, &invariant.deny_in) {
        return None;
    }
    let direct = fact
        .effects
        .iter()
        .any(|observed| observed.to_string() == effect);
    let denied = invariant.deny_in.join(", ");
    let mut trace = Vec::new();
    let how = if direct {
        trace.push(TraceStep {
            fact:   format!("fn `{}` performs `{effect}`", fact.function),
            reason: "observed effect-sink call or block in the body".to_owned()
        });
        String::new()
    } else {
        let chain = effect_chain(facts, graph, fact_index, effect);
        for window in chain.windows(2) {
            let reason = edge_reason(graph, window[0], window[1]);
            trace.push(TraceStep {
                fact:   format!(
                    "fn `{}` calls fn `{}`",
                    facts[window[0]].function, facts[window[1]].function
                ),
                reason: reason.to_owned()
            });
        }
        if let Some(&sink) = chain.last() {
            trace.push(TraceStep {
                fact:   format!("fn `{}` performs `{effect}`", facts[sink].function),
                reason: "observed effect-sink call or block in the body".to_owned()
            });
            format!(" through `{}`", facts[sink].function)
        } else {
            String::new()
        }
    };
    let verb = if direct {
        "performs"
    } else {
        "transitively performs"
    };
    let detail = format!(
        "invariant violated: `{}` {verb} `{effect}`{how} inside module `{module}`, but the \
         effect is denied in [{denied}]",
        fact.function
    );
    trace.push(TraceStep {
        fact:   format!("fn `{}` lives in module `{module}`", fact.function),
        reason: "module graph maps its file to this path".to_owned()
    });
    trace.push(TraceStep {
        fact:   format!("denied in: [{denied}]"),
        reason: "module is covered by a denied prefix".to_owned()
    });
    Some((detail, trace))
}

/// States how one traversed call edge was established, so a derivation
/// never claims more certainty than its weakest hop: dispatch over
/// several implementors admits possibility, an edge only the SCIP index
/// resolved names the index as its source, and everything else is the
/// syntactic resolver's unique match.
fn edge_reason(graph: &CallGraph, from: usize, to: usize) -> &'static str {
    if graph.possibilistic(from, to) {
        "dispatch edge over several implementors — the effect is possible, not certain"
    } else if graph.indexed(from, to) {
        "name-resolved call edge, read from the SCIP index"
    } else {
        "resolved call edge"
    }
}

/// Checks the layer constraint of one invariant: functions under
/// `call-deny-from` must not reach functions under `call-deny-to`.
fn call_violation(
    invariant: &Invariant,
    facts: &[FunctionFacts],
    fact_index: usize,
    graph: &CallGraph,
    module: &str,
    module_of: &dyn Fn(usize) -> String
) -> Option<(String, Vec<TraceStep>)> {
    let from = invariant.call_deny_from.as_deref()?;
    let to = invariant.call_deny_to.as_deref()?;
    if !covered_by_any(module, std::slice::from_ref(&from.to_owned())) {
        return None;
    }
    let fact = facts.get(fact_index)?;
    let chain = reach_chain(facts, graph, fact_index, |target| {
        covered_by_any(
            &module_of(facts[target].source),
            std::slice::from_ref(&to.to_owned())
        )
    })?;
    let sink = *chain.last()?;
    let mut trace = Vec::new();
    for window in chain.windows(2) {
        trace.push(TraceStep {
            fact:   format!(
                "fn `{}` calls fn `{}`",
                facts[window[0]].function, facts[window[1]].function
            ),
            reason: edge_reason(graph, window[0], window[1]).to_owned()
        });
    }
    trace.push(TraceStep {
        fact:   format!("fn `{}` lives under `{to}`", facts[sink].function),
        reason: "module graph maps its file below the denied prefix".to_owned()
    });
    trace.push(TraceStep {
        fact:   format!("fn `{}` lives under `{from}`", fact.function),
        reason: "module graph maps its file below the constrained prefix".to_owned()
    });
    let direct = chain.len() == 2;
    let how = if direct {
        "calls"
    } else {
        "transitively reaches"
    };
    let detail = format!(
        "invariant violated: `{}` ({from}) {how} `{}` ({to}); the layer boundary forbids it",
        fact.function, facts[sink].function
    );
    Some((detail, trace))
}

/// Finds the shortest resolved call chain from `start` to any function
/// matching `target`, both endpoints included.
fn reach_chain(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    start: usize,
    target: impl Fn(usize) -> bool
) -> Option<Vec<usize>> {
    let mut parent: Vec<Option<usize>> = vec![None; facts.len()];
    let mut visited = vec![false; facts.len()];
    let mut queue = std::collections::VecDeque::from([start]);
    visited[start] = true;
    while let Some(current) = queue.pop_front() {
        if current != start && target(current) {
            let mut chain = vec![current];
            let mut cursor = current;
            while let Some(previous) = parent[cursor] {
                chain.push(previous);
                cursor = previous;
            }
            chain.reverse();
            return Some(chain);
        }
        for &next in graph.callees(current) {
            if !visited[next] {
                visited[next] = true;
                parent[next] = Some(current);
                queue.push_back(next);
            }
        }
    }
    None
}

/// Finds the shortest resolved call chain from `start` to a function that
/// performs `effect` directly, both endpoints included; empty when no such
/// chain exists.
fn effect_chain(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    start: usize,
    effect: &str
) -> Vec<usize> {
    let mut parent: Vec<Option<usize>> = vec![None; facts.len()];
    let mut visited = vec![false; facts.len()];
    let mut queue = std::collections::VecDeque::from([start]);
    visited[start] = true;
    while let Some(current) = queue.pop_front() {
        if current != start
            && facts[current]
                .effects
                .iter()
                .any(|observed| observed.to_string() == effect)
        {
            let mut chain = vec![current];
            let mut cursor = current;
            while let Some(previous) = parent[cursor] {
                chain.push(previous);
                cursor = previous;
            }
            chain.reverse();
            return chain;
        }
        for &next in graph.callees(current) {
            if !visited[next] {
                visited[next] = true;
                parent[next] = Some(current);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}

/// Returns whether `module` equals one of `prefixes` or sits below one,
/// `::`-aligned.
fn covered_by_any(module: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        module == prefix
            || (module.starts_with(prefix.as_str()) && module[prefix.len()..].starts_with("::"))
    })
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

    use rustmanifest_schema::{Invariant, Severity};

    use super::check_invariants;
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
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

    #[test]
    fn writer_outside_the_allowlist_is_reported_with_a_trace() {
        let findings = check_invariants(&wallet_sources(), &[balance_invariant()]);
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
        assert!(check_invariants(&sources, &[balance_invariant()]).is_empty());
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
        let findings = check_invariants(&sources, &[net_ban("crate::domain")]);
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
        let findings = check_invariants(&sources, &[net_ban("crate::domain")]);
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
        let findings = check_invariants(&sources, &[net_ban("crate::domain")]);
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
        let findings = check_invariants(&sources, &[net_ban("crate::domain")]);
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
        let findings = check_invariants(&sources, &[invariant]);
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
        assert!(check_invariants(&sources, &[balance_invariant()]).is_empty());
    }

    #[test]
    fn no_invariants_means_no_work() {
        assert!(check_invariants(&wallet_sources(), &[]).is_empty());
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
        let findings = check_invariants(&sources, &[invariant]);
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
        assert!(check_invariants(&sources, &[invariant]).is_empty());
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
        let findings = check_invariants(&sources, &[invariant]);
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
        assert!(check_invariants(&sources, &[invariant]).is_empty());
    }
}
