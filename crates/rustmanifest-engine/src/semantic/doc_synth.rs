// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Deterministic rustdoc synthesis: a ready-to-paste `///` draft derived
//! from behavioral facts through hand-written community-formula
//! templates.
//!
//! The summary verb comes from the derived role, the object from the
//! name's own words; `# Errors` enumerates the signature's error types;
//! `# Panics` appears when the body provably may panic
//! (`unwrap`/`expect` call sites); `# Safety` when the function is
//! `unsafe`. Same facts, same draft — no model anywhere.

use super::{
    behavior::{Effect, FunctionFacts, ParamMode},
    graph::TypeGraph,
    role::{Role, role}
};

/// Renders a rustdoc draft for one function from its facts.
///
/// One `///` line per entry; field references resolve through the type
/// graph into linkable `[Type::field]` doc paths when the field provably
/// exists.
#[must_use]
pub fn doc_draft(fact: &FunctionFacts, graph: &TypeGraph) -> String {
    let mut lines: Vec<String> = vec![summary(fact, graph)];
    let arguments: Vec<String> = fact
        .params
        .iter()
        .filter(|param| param.name != "self")
        .map(|param| {
            let mode = match param.mode {
                ParamMode::Owned => "taken by value",
                ParamMode::Borrowed => "borrowed",
                ParamMode::BorrowedMut => "mutably borrowed"
            };
            format!("* `{}` — `{}`, {mode}.", param.name, param.type_name)
        })
        .collect();
    if !arguments.is_empty() {
        lines.push(String::new());
        lines.push("# Arguments".to_owned());
        lines.push(String::new());
        lines.extend(arguments);
    }
    if !fact.error_types.is_empty() {
        lines.push(String::new());
        lines.push("# Errors".to_owned());
        lines.push(String::new());
        for error in &fact.error_types {
            lines.push(format!(
                "Returns [`{error}`] when the operation cannot complete."
            ));
        }
    }
    if may_panic(fact) {
        lines.push(String::new());
        lines.push("# Panics".to_owned());
        lines.push(String::new());
        lines.push("Panics when an internal `unwrap` or `expect` fails.".to_owned());
    }
    if fact.effects.contains(&Effect::Unsafe) {
        lines.push(String::new());
        lines.push("# Safety".to_owned());
        lines.push(String::new());
        lines.push(
            "The caller must uphold the invariants required by the `unsafe` operations inside."
                .to_owned()
        );
    }
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                "///".to_owned()
            } else {
                format!("/// {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Summary derived from what the body provably does — the code is the
/// source of truth, never the name.
fn summary(fact: &FunctionFacts, graph: &TypeGraph) -> String {
    match role(fact) {
        Role::Constructor => fact
            .constructs
            .first()
            .cloned()
            .or_else(|| {
                fact.builds
                    .first()
                    .and_then(|built| built.split_once('.'))
                    .map(|(owner, _)| owner.to_owned())
            })
            .map_or_else(
                || "Creates a new value.".to_owned(),
                |constructed| format!("Creates a new [`{constructed}`].")
            ),
        Role::Accessor => fact.reads.first().map_or_else(
            || "Returns a value derived from the current state.".to_owned(),
            |read| {
                format!(
                    "Returns the current value of {}.",
                    field_reference(graph, read)
                )
            }
        ),
        Role::Mutator => {
            let written: Vec<String> = fact
                .writes
                .iter()
                .map(|write| field_reference(graph, write))
                .collect();
            format!("Updates {}.", written.join(", "))
        }
        Role::Io => {
            let mut touched: Vec<&str> = fact
                .effects
                .iter()
                .filter_map(|effect| match effect {
                    Effect::FileSystem => Some("the filesystem"),
                    Effect::Network => Some("the network"),
                    Effect::Process => Some("child processes"),
                    Effect::Environment => Some("the process environment"),
                    Effect::ConsoleIo => Some("the console"),
                    Effect::DatabaseRead | Effect::DatabaseWrite => Some("the database"),
                    Effect::Unsafe => None
                })
                .collect();
            touched.dedup();
            format!("Interacts with {}.", touched.join(" and "))
        }
        Role::Pure | Role::Unknown => "Computes a value from its inputs.".to_owned()
    }
}

/// Resolves an `Owner.field` fact key through the type graph: a field
/// that provably exists becomes a linkable `[Owner::field]` rustdoc
/// path; anything unresolved stays a plain code span, never a fake link.
fn field_reference(graph: &TypeGraph, key: &str) -> String {
    if let Some((owner, field)) = key.split_once('.')
        && graph
            .type_named(owner)
            .is_some_and(|node| node.fields.iter().any(|entry| entry.name == field))
    {
        return format!("[`{owner}::{field}`]");
    }
    format!("`{key}`")
}

/// Returns whether the body provably may panic through `unwrap` or
/// `expect` call sites.
fn may_panic(fact: &FunctionFacts) -> bool {
    fact.calls.iter().any(|call| {
        call == "unwrap"
            || call == "expect"
            || call.ends_with("::unwrap")
            || call.ends_with("::expect")
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

    use super::doc_draft;
    use crate::{semantic::behavior, source::Source};

    fn draft(text: &str, name: &str) -> String {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let graph = crate::semantic::TypeGraph::from_sources(&sources);
        doc_draft(
            facts.iter().find(|fact| fact.function == name).unwrap(),
            &graph
        )
    }

    #[test]
    fn accessor_draft_describes_the_observed_read() {
        let text = "struct Wallet { balance: i64 }\nimpl Wallet { fn get_balance(&self) -> i64 \
                    { self.balance } }";
        assert_eq!(
            draft(text, "Wallet::get_balance"),
            "/// Returns the current value of [`Wallet::balance`]."
        );
    }

    #[test]
    fn summary_ignores_a_lying_name_and_trusts_the_body() {
        let text = "struct Counter { hits: u64 }\nimpl Counter { fn get_hits(&mut self) -> u64 \
                    { self.hits += 1; self.hits } }";
        assert_eq!(
            draft(text, "Counter::get_hits"),
            "/// Updates [`Counter::hits`]."
        );
    }

    #[test]
    fn assemblers_document_the_built_type_not_an_update() {
        let text = "struct Order { items: u32 }\nimpl Order { fn empty() -> Self { Self { items: \
                    0 } } }\nstruct Repo;\nimpl Repo { fn fetch(&self) -> Order { let mut order \
                    = Order::empty(); order.items = 3; order } }";
        assert_eq!(draft(text, "Repo::fetch"), "/// Creates a new [`Order`].");
    }

    #[test]
    fn fallible_panicking_function_gets_both_sections() {
        let text = "fn load_config(path: &str) -> Result<u64, ConfigError> { \
                    std::fs::read(path).unwrap(); Ok(1) }";
        let rendered = draft(text, "load_config");
        assert!(rendered.contains("/// # Errors"));
        assert!(rendered.contains("[`ConfigError`]"));
        assert!(rendered.contains("/// # Panics"));
    }

    #[test]
    fn arguments_come_from_ownership_modes() {
        let text = "struct Ledger { total: u64 }\nfn post(ledger: &mut Ledger, amount: u64) { \
                    ledger.total += amount; }";
        let rendered = draft(text, "post");
        assert!(rendered.contains("/// # Arguments"));
        assert!(rendered.contains("* `ledger` — `Ledger`, mutably borrowed."));
        assert!(rendered.contains("* `amount` — `u64`, taken by value."));
    }

    #[test]
    fn drafts_are_deterministic() {
        let text = "fn compute_total(values: &[u64]) -> u64 { values.iter().sum() }";
        assert_eq!(draft(text, "compute_total"), draft(text, "compute_total"));
        assert_eq!(
            draft(text, "compute_total"),
            "/// Computes a value from its inputs.\n///\n/// # Arguments\n///\n/// * `values` — \
             `[u64]`, borrowed."
        );
    }
}
