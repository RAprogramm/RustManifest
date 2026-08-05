// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a resolution states, as plain data.
//!
//! The overlay is deliberately dull: a list of positions, each with the
//! function the call reaches and the type it hands back. Nothing here knows
//! how the answers were produced, which is what lets the engine consume
//! them without taking on the resolver.
//!
//! A position the resolver could not settle is **absent**. Absence is the
//! only encoding of "not answered" this shape has, so a consumer cannot
//! read an empty string as a resolved name.

use rustmanifest_schema::Location;
use serde::{Deserialize, Serialize};

/// One call the resolver settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedCall {
    /// File and byte range of the call expression.
    pub at:       Location,
    /// Fully qualified path of the function the call reaches.
    pub callee:   String,
    /// Type the call hands back, as the resolver renders it.
    pub returns:  String,
    /// Type of the receiver, where the call has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>
}

/// Everything one resolution run settled about one workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Resolution {
    /// Version of the resolver that produced it, so a stale overlay is
    /// recognisable rather than silently trusted.
    pub resolver: String,
    /// Calls the resolver settled, in file then position order.
    pub calls:    Vec<ResolvedCall>
}

impl Resolution {
    /// Returns the call settled at exactly this location, or `None` where
    /// the resolver settled none.
    #[must_use]
    pub fn at(&self, location: &Location) -> Option<&ResolvedCall> {
        self.calls.iter().find(|call| &call.at == location)
    }

    /// Sorts the calls into the order the shape promises, so two runs over
    /// the same workspace produce the same bytes.
    pub fn settle(&mut self) {
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
    use super::*;

    fn located(file: &str, start: u32) -> Location {
        Location {
            file:  std::path::PathBuf::from(file),
            range: rustmanifest_schema::Range {
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
    fn a_settled_call_is_found_by_its_position() {
        let resolution = Resolution {
            resolver: "test".to_owned(),
            calls:    vec![call("src/lib.rs", 10, "core::option::Option::is_some")]
        };
        let found = resolution
            .at(&located("src/lib.rs", 10))
            .expect("the call is settled");
        assert_eq!(found.callee, "core::option::Option::is_some");
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
