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

use rustmanifest_schema::{Finding, Invariant, Location, Severity, TraceStep};

use super::{behavior::behavior, module_overview::module_overview};
use crate::{pragma, source::Source};

/// Checks every invariant over `sources`, honoring pragma suppression in
/// the file each finding anchors to.
///
/// Findings carry the invariant's `id` as their rule id, or a positional
/// `RM-INV-NNN` when the entry declares none; severity defaults to
/// [`Severity::Error`].
#[must_use]
pub fn check_invariants(sources: &[Source], invariants: &[Invariant]) -> Vec<Finding> {
    if invariants.is_empty() {
        return Vec::new();
    }
    let facts = behavior(sources);
    let overview = module_overview(sources);
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
        for fact in &facts {
            let module = module_of(fact.source);
            let violation = field_violation(invariant, fact, &module)
                .or_else(|| effect_violation(invariant, fact, &module));
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
                trace
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
/// function's facts.
fn effect_violation(
    invariant: &Invariant,
    fact: &super::behavior::FunctionFacts,
    module: &str
) -> Option<(String, Vec<TraceStep>)> {
    let effect = invariant.effect.as_deref()?;
    if !fact
        .effects
        .iter()
        .any(|observed| observed.to_string() == effect)
    {
        return None;
    }
    if !covered_by_any(module, &invariant.deny_in) {
        return None;
    }
    let denied = invariant.deny_in.join(", ");
    let detail = format!(
        "invariant violated: `{}` performs `{effect}` inside module `{module}`, but the effect is \
         denied in [{denied}]",
        fact.function
    );
    let trace = vec![
        TraceStep {
            fact:   format!("fn `{}` performs `{effect}`", fact.function),
            reason: "observed effect-sink call or block in the body".to_owned()
        },
        TraceStep {
            fact:   format!("fn `{}` lives in module `{module}`", fact.function),
            reason: "module graph maps its file to this path".to_owned()
        },
        TraceStep {
            fact:   format!("denied in: [{denied}]"),
            reason: "module is covered by a denied prefix".to_owned()
        },
    ];
    Some((detail, trace))
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
}
