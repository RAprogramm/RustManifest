// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Ecosystem knowledge packs: what the engine knows about well-known
//! third-party crates, encoded as data — one module per crate.
//!
//! A pack declares the crate's constructor functions and method return
//! types, and the effect its calls carry — at method precision, so a
//! pure accessor on an IO-owning type never inherits an effect it does
//! not have. Packs grow by adding a line and never guess: an entry
//! states what the crate's own documentation guarantees. Workspace
//! facts always win — a pack is consulted only when the workspace
//! itself has no answer.

mod reqwest;
mod result;
mod statement;
mod tempfile;
mod tokio;

pub(super) use self::{
    result::{MethodResult, Outcome, ReturnForm, converts},
    statement::{statement_of, table_of}
};
use super::behavior::Effect;

/// Everything one pack knows about one crate.
pub(super) struct CrateKnowledge {
    /// Crate name as it appears in call paths.
    pub name:           &'static str,
    /// Constructor call names with the concrete type each produces;
    /// `Result`/`Option` wrappers already unwrapped, since these are
    /// consumed behind `unwrap`/`?`.
    pub constructors:   &'static [(&'static str, &'static str)],
    /// `(type, method, returned type)` rows typing the crate's builder
    /// and accessor chains, wrappers unwrapped the same way.
    pub method_returns: &'static [(&'static str, &'static str, &'static str)],
    /// `(type, method, effect)` rows for the calls that actually
    /// perform the effect.
    pub method_effects: &'static [(&'static str, &'static str, Effect)],
    /// Effect of free-function calls rooted in the crate's own name,
    /// e.g. a one-shot helper at the crate root.
    pub crate_effect:   Option<Effect>
}

/// Every registered pack.
const PACKS: [&CrateKnowledge; 3] = [&reqwest::PACK, &tempfile::PACK, &tokio::PACK];

/// Looks up the type produced by a known external constructor call.
pub(super) fn constructor_type(name: &str) -> Option<&'static str> {
    PACKS
        .iter()
        .flat_map(|pack| pack.constructors)
        .find(|(constructor, _)| *constructor == name)
        .map(|(_, produced)| *produced)
}

/// Looks up the return type of a pack-known method on a pack-known type.
pub(super) fn method_return(owner: &str, method: &str) -> Option<&'static str> {
    PACKS
        .iter()
        .flat_map(|pack| pack.method_returns)
        .find(|(known_owner, known_method, _)| *known_owner == owner && *known_method == method)
        .map(|(_, _, returned)| *returned)
}

/// Looks up the effect performed by a pack-known method call.
pub(super) fn method_effect(owner: &str, method: &str) -> Option<Effect> {
    PACKS
        .iter()
        .flat_map(|pack| pack.method_effects)
        .find(|(known_owner, known_method, _)| *known_owner == owner && *known_method == method)
        .map(|(_, _, effect)| *effect)
}

/// Projection methods of the pin-project family: the returned view's
/// named fields mirror the receiver's own fields, so for field typing
/// the projection is the receiver. Every expansion names its view the
/// same, which collapses the unique-name field table — the reason this
/// is knowledge and not inference.
pub(super) const PROJECTION_METHODS: [&str; 2] = ["project", "project_ref"];

/// View type names the pin-project family generates, fixed by the
/// macro itself for every expansion.
pub(super) const PROJECTION_VIEWS: [&str; 2] = ["Projection", "ProjectionRef"];

/// The `tower` service entry point: layering wrappers preserve it, so
/// `call` on a wrapper whose wrapped service type is known dispatches
/// to that service's own `call`.
pub(super) const SERVICE_CALL: &str = "call";

/// Whether a call by these segments is one of the `sqlx` crate's
/// query constructors: the crate's own contract says the first
/// argument is a statement headed for the database, whatever built it.
pub(super) fn sql_call(segments: &[String]) -> bool {
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return false;
    };
    first == "sqlx" && SQL_MACROS.contains(&last.as_str())
}

/// SQL-carrying macro names of the `sqlx` crate.
const SQL_MACROS: [&str; 6] = [
    "query",
    "query_as",
    "query_as_unchecked",
    "query_scalar",
    "query_scalar_unchecked",
    "query_unchecked"
];

/// Classifies a SQL-carrying macro invocation into a database effect by
/// the first keyword of its first string literal. Statements whose kind
/// is not provable from the keyword yield nothing — silence over
/// guessing.
pub(super) fn sql_macro_effect(macro_name: &str, tokens: &str) -> Option<Effect> {
    if !SQL_MACROS.contains(&macro_name) {
        return None;
    }
    statement_effect(tokens.split_once('"')?.1)
}

/// Classifies a SQL-carrying function call of the `sqlx` crate the same
/// way, where the SQL it is handed is written at the call. The function
/// names are the macro names: the crate exports both forms over one
/// statement, differing in when the statement is checked, not in what
/// it does.
pub(super) fn sql_call_effect(segments: &[String], sql: &str) -> Option<Effect> {
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return None;
    };
    if first != "sqlx" || !SQL_MACROS.contains(&last.as_str()) {
        return None;
    }
    statement_effect(sql)
}

/// The database effect the first keyword of a statement proves.
fn statement_effect(sql: &str) -> Option<Effect> {
    let keyword: String = sql
        .trim_start()
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_uppercase();
    match keyword.as_str() {
        "SELECT" => Some(Effect::DatabaseRead),
        "ALTER" | "CREATE" | "DELETE" | "DROP" | "INSERT" | "TRUNCATE" | "UPDATE" => {
            Some(Effect::DatabaseWrite)
        }
        _ => None
    }
}

/// Looks up the effect of a free call rooted in a pack's crate name.
pub(super) fn crate_effect(first_segment: &str) -> Option<Effect> {
    PACKS
        .iter()
        .find(|pack| pack.name == first_segment)
        .and_then(|pack| pack.crate_effect)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{
        super::behavior::Effect, constructor_type, crate_effect, method_effect, method_return
    };

    #[test]
    fn constructors_resolve_to_their_produced_types() {
        assert_eq!(constructor_type("tempdir"), Some("TempDir"));
        assert_eq!(constructor_type("unknown_fn"), None);
    }

    #[test]
    fn builder_chains_type_through_method_returns() {
        assert_eq!(method_return("Client", "get"), Some("RequestBuilder"));
        assert_eq!(method_return("RequestBuilder", "send"), Some("Response"));
        assert_eq!(
            method_return("TempDir", "path"),
            Some("Path"),
            "a bed hands back a path, and every call written on it is typed by that"
        );
        assert_eq!(method_return("Command", "arg"), Some("Command"));
        assert_eq!(method_return("Command", "spawn"), Some("Child"));
        assert_eq!(method_return("Child", "wait"), Some("ExitStatus"));
    }

    #[test]
    fn only_performing_methods_carry_effects() {
        assert_eq!(
            method_effect("RequestBuilder", "send"),
            Some(Effect::Network)
        );
        assert_eq!(method_effect("Client", "get"), None);
        assert_eq!(method_effect("TempDir", "path"), None);
        assert_eq!(method_effect("TempDir", "close"), Some(Effect::FileSystem));
        assert_eq!(
            method_effect("TcpListener", "accept"),
            Some(Effect::Network)
        );
        assert_eq!(
            method_effect("TcpStream", "write_all"),
            Some(Effect::Network)
        );
        assert_eq!(method_effect("Command", "output"), Some(Effect::Process));
        assert_eq!(method_effect("Command", "arg"), None);
    }

    #[test]
    fn crate_roots_carry_their_one_shot_effect() {
        assert_eq!(crate_effect("reqwest"), Some(Effect::Network));
        assert_eq!(crate_effect("serde"), None);
        assert_eq!(crate_effect("tokio"), None);
    }
}
