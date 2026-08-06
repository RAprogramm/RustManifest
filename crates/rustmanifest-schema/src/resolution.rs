// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a name-and-type resolver settled about a workspace, as plain data.
//!
//! The shape lives here, in the crate every part of the engine already
//! depends on, so that the engine can read a resolution without depending
//! on whatever produced it. The producer is a separate crate carrying the
//! rust-analyzer libraries; nothing of that reaches this far.
//!
//! A position the resolver could not settle is **absent**. Absence is the
//! only encoding of "not answered" this shape has, so a consumer cannot
//! read an empty string as a resolved name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Location;

/// One call a resolver settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedCall {
    /// File and byte range of the call expression.
    pub at:       Location,
    /// Name of the function the call reaches.
    pub callee:   String,
    /// Type the call hands back, as the resolver renders it.
    pub returns:  String,
    /// Type of the receiver, where the call has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>
}

/// One field access a resolver settled.
///
/// The engine reads a field by naming the type that holds it, and a type
/// outside the workspace is one the written text cannot name. This row
/// answers exactly that: which type the value being read from has, and
/// which type the field itself has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedField {
    /// File and byte range of the field expression.
    pub at:       Location,
    /// Type of the value the field is read out of.
    pub owner:    String,
    /// Type the field itself has, as the resolver renders it.
    pub produced: String
}

/// Everything one resolution run settled about one workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Resolution {
    /// Version of the resolver that produced it, so a stale overlay is
    /// recognisable rather than silently trusted.
    pub resolver: String,
    /// Calls the resolver settled, in file then position order.
    pub calls:    Vec<ResolvedCall>,
    /// Field accesses the resolver settled, in file then position order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields:   Vec<ResolvedField>
}

impl Resolution {
    /// Returns the call settled at exactly this location, or `None` where
    /// the resolver settled none.
    #[must_use]
    pub fn at(&self, location: &Location) -> Option<&ResolvedCall> {
        self.calls.iter().find(|call| &call.at == location)
    }

    /// Sorts the calls into the order the shape promises and drops exact
    /// repeats, so two runs over the same workspace produce the same bytes.
    pub fn settle(&mut self) {
        self.fields.sort_by(|left, right| {
            left.at
                .file
                .cmp(&right.at.file)
                .then(left.at.range.start.cmp(&right.at.range.start))
                .then(left.at.range.end.cmp(&right.at.range.end))
        });
        self.fields.dedup();
        self.calls.sort_by(|left, right| {
            left.at
                .file
                .cmp(&right.at.file)
                .then(left.at.range.start.cmp(&right.at.range.start))
                .then(left.at.range.end.cmp(&right.at.range.end))
                .then(left.callee.cmp(&right.callee))
        });
        self.calls.dedup();
    }
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

    use super::*;
    use crate::Range;

    fn located(file: &str, start: u32) -> Location {
        Location {
            file:  PathBuf::from(file),
            range: Range {
                start,
                end: start + 4
            }
        }
    }

    fn call(file: &str, start: u32, callee: &str) -> ResolvedCall {
        ResolvedCall {
            at:       located(file, start),
            callee:   callee.to_owned(),
            returns:  "bool".to_owned(),
            receiver: None
        }
    }

    #[test]
    fn a_settled_call_is_found_by_its_location() {
        let resolution = Resolution {
            resolver: "test".to_owned(),
            calls:    vec![call("src/lib.rs", 10, "is_some")]
        };
        let found = resolution
            .at(&located("src/lib.rs", 10))
            .expect("the call is settled");
        assert_eq!(found.callee, "is_some");
        assert_eq!(found.returns, "bool");
    }

    #[test]
    fn a_position_the_resolver_did_not_settle_is_absent() {
        let resolution = Resolution {
            resolver: "test".to_owned(),
            calls:    vec![call("src/lib.rs", 10, "go")]
        };
        assert!(resolution.at(&located("src/lib.rs", 99)).is_none());
    }

    #[test]
    fn settling_orders_by_file_then_position_and_drops_repeats() {
        let mut resolution = Resolution {
            resolver: "test".to_owned(),
            calls:    vec![
                call("src/z.rs", 1, "z"),
                call("src/a.rs", 20, "b"),
                call("src/a.rs", 5, "a"),
                call("src/a.rs", 5, "a"),
            ]
        };
        resolution.settle();
        let order: Vec<&str> = resolution
            .calls
            .iter()
            .map(|entry| entry.callee.as_str())
            .collect();
        assert_eq!(order, ["a", "b", "z"]);
    }

    #[test]
    fn an_unresolved_receiver_is_left_out_of_the_json() {
        let text = serde_json::to_string(&call("src/lib.rs", 0, "go")).unwrap();
        assert!(!text.contains("receiver"));
    }
}
