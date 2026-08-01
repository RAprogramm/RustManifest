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
    behavior::{Effect, FunctionFacts},
    role::{Role, role}
};

/// Renders a rustdoc draft for one function from its facts, one `///`
/// line per entry.
#[must_use]
pub fn doc_draft(fact: &FunctionFacts) -> String {
    let mut lines: Vec<String> = vec![summary(fact)];
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
fn summary(fact: &FunctionFacts) -> String {
    match role(fact) {
        Role::Constructor => fact.constructs.first().map_or_else(
            || "Creates a new value.".to_owned(),
            |constructed| format!("Creates a new [`{constructed}`].")
        ),
        Role::Accessor => fact.reads.first().map_or_else(
            || "Returns a value derived from the current state.".to_owned(),
            |read| format!("Returns the current value of `{read}`.")
        ),
        Role::Mutator => {
            let written: Vec<String> = fact
                .writes
                .iter()
                .map(|write| format!("`{write}`"))
                .collect();
            format!("Updates {}.", written.join(", "))
        }
        Role::Io => {
            let touched: Vec<&str> = fact
                .effects
                .iter()
                .filter_map(|effect| match effect {
                    Effect::FileSystem => Some("the filesystem"),
                    Effect::Network => Some("the network"),
                    Effect::Process => Some("child processes"),
                    Effect::Environment => Some("the process environment"),
                    Effect::ConsoleIo => Some("the console"),
                    Effect::Unsafe => None
                })
                .collect();
            format!("Interacts with {}.", touched.join(" and "))
        }
        Role::Pure | Role::Unknown => "Computes a value from its inputs.".to_owned()
    }
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
        let facts = behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]);
        doc_draft(facts.iter().find(|fact| fact.function == name).unwrap())
    }

    #[test]
    fn accessor_draft_describes_the_observed_read() {
        let text = "struct Wallet { balance: i64 }\nimpl Wallet { fn get_balance(&self) -> i64 \
                    { self.balance } }";
        assert_eq!(
            draft(text, "Wallet::get_balance"),
            "/// Returns the current value of `Wallet.balance`."
        );
    }

    #[test]
    fn summary_ignores_a_lying_name_and_trusts_the_body() {
        let text = "struct Counter { hits: u64 }\nimpl Counter { fn get_hits(&mut self) -> u64 \
                    { self.hits += 1; self.hits } }";
        assert_eq!(
            draft(text, "Counter::get_hits"),
            "/// Updates `Counter.hits`."
        );
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
    fn drafts_are_deterministic() {
        let text = "fn compute_total(values: &[u64]) -> u64 { values.iter().sum() }";
        assert_eq!(draft(text, "compute_total"), draft(text, "compute_total"));
        assert_eq!(
            draft(text, "compute_total"),
            "/// Computes a value from its inputs."
        );
    }
}
