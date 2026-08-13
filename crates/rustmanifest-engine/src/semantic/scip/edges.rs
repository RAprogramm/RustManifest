// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Caller-keyed call edges of one SCIP index and their binding to facts.
//!
//! Loading an index yields a map from `(document path, caller name)` to
//! the callees referenced inside that caller's body. Asking the map for
//! candidates turns each recorded callee into a fact index: a unique
//! short name binds directly, a colliding one is settled by the symbol's
//! full identity, and an unsettled collision is dropped.

use std::{collections::BTreeMap, path::Path};

use super::{
    definition::{DEFINITION_ROLE, function_definitions, range_start},
    identity,
    symbol::{Callee, callee, function_symbol},
    wire
};
use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Failure to load a SCIP index.
#[derive(Debug, thiserror::Error)]
pub enum ScipError {
    /// The index file could not be read.
    #[error("cannot read SCIP index: {0}")]
    Read(#[from] std::io::Error),
    /// The file's bytes are not a SCIP protobuf index.
    #[error("cannot decode SCIP index: truncated or malformed protobuf")]
    Decode
}

/// Name-resolved call edges of one SCIP index, keyed by caller.
#[derive(Debug, Default)]
pub struct ScipEdges {
    /// `(relative path, caller qualified name)` to callees.
    by_caller: BTreeMap<(String, String), Vec<Callee>>
}

impl ScipEdges {
    /// Reads and decodes an index file into caller-keyed call edges.
    ///
    /// # Errors
    ///
    /// Returns [`ScipError::Read`] when the file cannot be read and
    /// [`ScipError::Decode`] when its bytes are not a SCIP index.
    pub fn from_file(path: &Path) -> Result<Self, ScipError> {
        let bytes = std::fs::read(path)?;
        let documents = wire::documents(&bytes).ok_or(ScipError::Decode)?;
        Ok(Self::from_documents(&documents))
    }

    /// Builds caller-keyed edges from decoded documents.
    #[must_use]
    fn from_documents(documents: &[wire::Document]) -> Self {
        let mut by_caller: BTreeMap<(String, String), Vec<Callee>> = BTreeMap::new();
        for document in documents {
            let definitions = function_definitions(document);
            for occurrence in &document.occurrences {
                if occurrence.symbol_roles & DEFINITION_ROLE != 0
                    || !function_symbol(&occurrence.symbol)
                {
                    continue;
                }
                let Some(position) = range_start(&occurrence.range) else {
                    continue;
                };
                let caller = definitions
                    .iter()
                    .filter(|definition| definition.contains(position))
                    .min_by_key(|definition| definition.extent());
                if let Some(definition) = caller {
                    by_caller
                        .entry((document.relative_path.clone(), definition.name.clone()))
                        .or_default()
                        .push(callee(&occurrence.symbol));
                }
            }
        }
        for callees in by_caller.values_mut() {
            callees.sort();
            callees.dedup();
        }
        Self {
            by_caller
        }
    }

    /// Callees recorded for the function `caller` in the document whose
    /// relative path ends the given source path.
    fn callees(&self, source_path: &str, caller: &str) -> &[Callee] {
        self.by_caller
            .iter()
            .find(|((path, name), _)| name == caller && path_matches(source_path, path))
            .map_or(&[], |(_, callees)| callees.as_slice())
    }

    /// Resolves each function's SCIP callees to fact indices.
    ///
    /// A callee whose short name is unique among the facts binds
    /// directly. A colliding short name — the same `Type::method`
    /// defined by more than one analyzed crate — is settled by the
    /// symbol's full identity: the crate name against the crate the
    /// fact's file belongs to, and the descriptor path against the
    /// fact's module path. A collision the full identity cannot settle
    /// is dropped, never guessed.
    #[must_use]
    pub fn candidates(&self, facts: &[FunctionFacts], sources: &[Source]) -> Vec<Vec<usize>> {
        let mut by_short: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (index, fact) in facts.iter().enumerate() {
            by_short.entry(&fact.function).or_default().push(index);
        }
        let identities = identity::of_sources(sources, None);
        facts
            .iter()
            .map(|fact| {
                let Some(source) = sources.get(fact.source) else {
                    return Vec::new();
                };
                self.callees(&source.path().to_string_lossy(), &fact.function)
                    .iter()
                    .filter_map(|callee| resolve(callee, &by_short, facts, &identities))
                    .collect()
            })
            .collect()
    }
}

/// Binds one callee to a fact index: directly when the short name is
/// unique, through the full symbol identity when it collides.
fn resolve(
    callee: &Callee,
    by_short: &BTreeMap<&str, Vec<usize>>,
    facts: &[FunctionFacts],
    identities: &[identity::SourceIdentity]
) -> Option<usize> {
    let matches = by_short.get(callee.short.as_str())?;
    if let [only] = matches.as_slice() {
        return Some(*only);
    }
    let mut settled = matches.iter().filter(|&&index| {
        identities
            .get(facts[index].source)
            .is_some_and(|identity| identity.matches(callee, &facts[index].function))
    });
    let first = settled.next()?;
    settled.next().is_none().then_some(*first)
}

/// Returns whether a workspace source path ends with the index's
/// relative document path, seen with forward slashes.
fn path_matches(source_path: &str, relative: &str) -> bool {
    let normalized = source_path.replace('\\', "/");
    let normalized = normalized.trim_start_matches("./");
    normalized == relative || normalized.ends_with(&format!("/{relative}"))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{DEFINITION_ROLE, ScipEdges, wire};

    fn put_varint(out: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = u8::try_from(value & 0x7f).unwrap();
            value >>= 7;
            if value == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
    }

    fn put_field(out: &mut Vec<u8>, field: u64, payload: &[u8]) {
        put_varint(out, (field << 3) | 2);
        put_varint(out, payload.len() as u64);
        out.extend_from_slice(payload);
    }

    fn put_varint_field(out: &mut Vec<u8>, field: u64, value: u64) {
        put_varint(out, field << 3);
        put_varint(out, value);
    }

    fn packed(values: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        for value in values {
            put_varint(&mut out, u64::from(u32::try_from(*value).unwrap()));
        }
        out
    }

    fn occurrence(symbol: &str, range: &[i32], roles: i32, enclosing: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        put_field(&mut out, 1, &packed(range));
        put_field(&mut out, 2, symbol.as_bytes());
        if roles != 0 {
            put_varint_field(&mut out, 3, u64::from(u32::try_from(roles).unwrap()));
        }
        if !enclosing.is_empty() {
            put_field(&mut out, 7, &packed(enclosing));
        }
        out
    }

    fn index() -> Vec<u8> {
        let mut document = Vec::new();
        put_field(&mut document, 1, b"src/lib.rs");
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo demo 0.1.0 lib/caller().",
                &[0, 3, 9],
                DEFINITION_ROLE,
                &[0, 0, 4, 1]
            )
        );
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo demo 0.1.0 lib/Wallet#credit().",
                &[2, 4, 10],
                0,
                &[]
            )
        );
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo std https://github.com/rust-lang/rust/library/std \
                 fs/remove_dir_all().",
                &[3, 4, 18],
                0,
                &[]
            )
        );
        let mut out = Vec::new();
        put_field(&mut out, 2, &document);
        out
    }

    #[test]
    fn references_attribute_to_their_enclosing_definition() {
        let documents = wire::documents(&index()).unwrap();
        let edges = ScipEdges::from_documents(&documents);
        let shorts: Vec<&str> = edges
            .callees("./src/lib.rs", "caller")
            .iter()
            .map(|callee| callee.short.as_str())
            .collect();
        assert_eq!(shorts, ["Wallet::credit", "remove_dir_all"]);
        assert!(edges.callees("./src/lib.rs", "unknown").is_empty());
        assert!(edges.callees("./other.rs", "caller").is_empty());
    }

    #[test]
    fn colliding_short_names_settle_through_crate_and_module_path() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [
            Source::new(
                PathBuf::from("tokio-1.53.1/src/net/tcp/stream.rs"),
                "pub struct TcpStream;\nimpl TcpStream { pub fn read(&self) {} }\npub fn \
                 caller(stream: &TcpStream) { stream.read(); }"
                    .to_owned()
            ),
            Source::new(
                PathBuf::from("mio-1.1.1/src/net/tcp/stream.rs"),
                "pub struct TcpStream;\nimpl TcpStream { pub fn read(&self) {} }".to_owned()
            )
        ];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let mut document = Vec::new();
        put_field(&mut document, 1, b"tokio-1.53.1/src/net/tcp/stream.rs");
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo tokio 1.53.1 net/tcp/stream/caller().",
                &[2, 7, 13],
                DEFINITION_ROLE,
                &[2, 0, 2, 60]
            )
        );
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo mio 1.1.1 net/tcp/stream/TcpStream#read().",
                &[2, 40, 44],
                0,
                &[]
            )
        );
        let mut bytes = Vec::new();
        put_field(&mut bytes, 2, &document);
        let documents = wire::documents(&bytes).unwrap();
        let edges = ScipEdges::from_documents(&documents);
        let overlay = edges.candidates(&facts, &sources);
        let caller = facts
            .iter()
            .position(|fact| fact.function == "caller")
            .unwrap();
        let mio_read = facts
            .iter()
            .position(|fact| fact.function == "TcpStream::read" && fact.source == 1)
            .unwrap();
        assert_eq!(overlay[caller], [mio_read]);
    }

    #[test]
    fn an_effect_ban_fires_through_an_edge_only_the_index_resolves() {
        use std::path::PathBuf;

        use rustmanifest_schema::Invariant;

        use crate::{
            semantic::{invariant::check_invariants_settled, module_graph::declared_tree},
            source::Source
        };
        let sources = [
            Source::new(
                PathBuf::from("src/lib.rs"),
                "mod domain;\nmod net;".to_owned()
            ),
            Source::new(
                PathBuf::from("src/net.rs"),
                "pub fn dial() { std::net::TcpStream::connect(\"h\").ok(); }".to_owned()
            ),
            Source::new(
                PathBuf::from("src/domain.rs"),
                "pub fn drive() { dispatch!(dial()); }".to_owned()
            )
        ];
        let ban = Invariant {
            id:              Some("NO-NET".to_owned()),
            field:           None,
            write_only_from: Vec::new(),
            effect:          Some("net".to_owned()),
            deny_in:         vec!["crate::domain".to_owned()],
            call_deny_from:  None,
            call_deny_to:    None,
            severity:        None
        };
        assert!(
            check_invariants_settled(
                &sources,
                std::slice::from_ref(&ban),
                None,
                Some(&declared_tree(&sources))
            )
            .is_empty(),
            "the macro-wrapped call is invisible to the syntactic resolver"
        );

        let mut document = Vec::new();
        put_field(&mut document, 1, b"src/domain.rs");
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo demo 0.1.0 domain/drive().",
                &[0, 7, 12],
                DEFINITION_ROLE,
                &[0, 0, 0, 36]
            )
        );
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo demo 0.1.0 net/dial().",
                &[0, 26, 30],
                0,
                &[]
            )
        );
        let mut bytes = Vec::new();
        put_field(&mut bytes, 2, &document);
        let edges = ScipEdges::from_documents(&wire::documents(&bytes).unwrap());

        let findings = check_invariants_settled(
            &sources,
            &[ban],
            Some(&edges),
            Some(&declared_tree(&sources))
        );
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0]
                .message
                .contains("`drive` transitively performs `net`")
        );
        let step = findings[0]
            .trace
            .iter()
            .find(|step| step.fact.contains("calls fn `dial`"))
            .expect("the index-owed edge is in the trace");
        assert_eq!(
            step.reason,
            "name-resolved call edge, read from the SCIP index"
        );
    }
}
