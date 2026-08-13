// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a name-and-type resolver settled about a workspace, as plain data.
//!
//! The shape lives here, in the crate every part of the engine already
//! depends on, so that the engine can read a resolution without depending
//! on whatever produced it. The producer is a separate crate carrying the
//! language-server libraries; nothing of that reaches this far.
//!
//! A position the resolver could not settle is **absent**. Absence is the
//! only encoding of "not answered" this shape has, so a consumer cannot
//! read an empty string as a resolved name.
//!
//! Four kinds of row travel together because they answer the four questions
//! a reading of written text cannot answer for itself: which function a call
//! reaches, which type holds a field, what type an expression has, and which
//! item a written name names. The last two are what let a reader stop
//! guessing from spelling — see [`typed`] and [`path`] for why each exists.

mod call;
mod declared_type;
mod function;
mod granted_trait;
mod layout;
mod module;
mod path;
mod positioned;
mod property;
mod signature;
mod typed;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::{
    call::{ResolvedCall, ResolvedField},
    declared_type::{ResolvedFieldDecl, ResolvedTypeDecl},
    function::ResolvedFunction,
    granted_trait::ResolvedGrant,
    layout::ResolvedLayout,
    module::ResolvedModule,
    path::ResolvedPath,
    property::ResolvedProperty,
    signature::{ResolvedMode, ResolvedParam, ResolvedSignature},
    typed::ResolvedType
};
use crate::Location;

/// Everything one resolution run settled about one workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Resolution {
    /// Version of the resolver that produced it, so a stale overlay is
    /// recognisable rather than silently trusted.
    pub resolver:      String,
    /// Calls the resolver settled, in file then position order.
    pub calls:         Vec<ResolvedCall>,
    /// Field accesses the resolver settled, in file then position order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields:        Vec<ResolvedField>,
    /// Types of expressions the resolver settled, in file then position
    /// order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types:         Vec<ResolvedType>,
    /// Written paths the resolver settled to an item, in file then position
    /// order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths:         Vec<ResolvedPath>,
    /// What the compiler grants each type the workspace mentions, in name
    /// order. Keyed by type rather than by position: the property belongs
    /// to the type, not to the place it was written at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types_granted: Vec<ResolvedProperty>,
    /// The compiler's own module tree of the workspace, in module-path
    /// order. Which file holds which module is the compiler's answer, not
    /// something a file-name reading reconstructs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules:       Vec<ResolvedModule>,
    /// Every function the workspace declares, in file then position order,
    /// named by the full path that reaches it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions:     Vec<ResolvedFunction>,
    /// What the compiler says each type the workspace mentions weighs, in
    /// name order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layouts:       Vec<ResolvedLayout>,
    /// Every type the workspace declares, with its fields, in file then
    /// position order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared:      Vec<ResolvedTypeDecl>,
    /// Every trait every type carries, in type then trait order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grants:        Vec<ResolvedGrant>,
    /// The signature of every declared function, in file then position
    /// order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures:    Vec<ResolvedSignature>
}

impl Resolution {
    /// Returns the call settled at exactly this location, or `None` where
    /// the resolver settled none.
    #[must_use]
    pub fn at(&self, location: &Location) -> Option<&ResolvedCall> {
        positioned::at(&self.calls, location)
    }

    /// Returns the field access settled at exactly this location.
    #[must_use]
    pub fn field_at(&self, location: &Location) -> Option<&ResolvedField> {
        positioned::at(&self.fields, location)
    }

    /// Returns the type of the expression written at exactly this location.
    #[must_use]
    pub fn type_at(&self, location: &Location) -> Option<&ResolvedType> {
        positioned::at(&self.types, location)
    }

    /// Returns the signature of the function whose name is written at
    /// exactly this location.
    #[must_use]
    pub fn signature_at(&self, location: &Location) -> Option<&ResolvedSignature> {
        positioned::at(&self.signatures, location)
    }

    /// Returns the function declared at exactly this location.
    #[must_use]
    pub fn function_at(&self, location: &Location) -> Option<&ResolvedFunction> {
        positioned::at(&self.functions, location)
    }

    /// Returns what the path written at exactly this location names.
    #[must_use]
    pub fn path_at(&self, location: &Location) -> Option<&ResolvedPath> {
        positioned::at(&self.paths, location)
    }

    /// Returns what the compiler says the type rendered as `named` weighs.
    #[must_use]
    pub fn weighs(&self, named: &str) -> Option<&ResolvedLayout> {
        self.layouts
            .binary_search_by(|row| row.named.as_str().cmp(named))
            .ok()
            .and_then(|found| self.layouts.get(found))
    }

    /// Returns what the compiler grants the type rendered as `named`.
    #[must_use]
    pub fn granted(&self, named: &str) -> Option<&ResolvedProperty> {
        self.types_granted
            .binary_search_by(|row| row.named.as_str().cmp(named))
            .ok()
            .and_then(|found| self.types_granted.get(found))
    }

    /// Whether the type rendered as `named` carries `trait_name`.
    #[must_use]
    pub fn carries(&self, named: &str, trait_name: &str) -> bool {
        self.grants
            .iter()
            .any(|row| row.named == named && row.trait_name == trait_name)
    }

    /// Puts every row on the path a reader names its file by.
    ///
    /// The shape states a file relative to the workspace root it was read
    /// from; a reader holds the same file however it walked to it. The two
    /// spellings are one file, and a row keyed by the other answers for
    /// nothing — so a reader rooted anywhere but that root says so once,
    /// here, instead of every lookup guessing.
    pub fn rebase(&mut self, root: &std::path::Path) {
        for row in &mut self.calls {
            row.at.file = root.join(&row.at.file);
        }
        for row in &mut self.fields {
            row.at.file = root.join(&row.at.file);
        }
        for row in &mut self.types {
            row.at.file = root.join(&row.at.file);
        }
        for row in &mut self.paths {
            row.at.file = root.join(&row.at.file);
        }
        for row in &mut self.functions {
            row.at.file = root.join(&row.at.file);
        }
        for row in &mut self.signatures {
            row.at.file = root.join(&row.at.file);
        }
        for row in &mut self.modules {
            row.file = root.join(&row.file);
        }
        self.settle();
    }

    /// Sorts every kind of row into the order the shape promises and drops
    /// exact repeats, so two runs over the same workspace produce the same
    /// bytes.
    ///
    /// This is also what establishes the order the lookups bisect, so a
    /// resolution assembled by hand is settled before it is read.
    pub fn settle(&mut self) {
        positioned::settle(&mut self.fields);
        positioned::settle(&mut self.calls);
        positioned::settle(&mut self.types);
        positioned::settle(&mut self.paths);
        positioned::settle(&mut self.functions);
        positioned::settle(&mut self.signatures);
        self.declared
            .sort_by(|left, right| left.at.file.cmp(&right.at.file).then(left.at.range.start.cmp(&right.at.range.start)));
        self.declared.dedup();
        self.grants
            .sort_by(|left, right| left.named.cmp(&right.named).then(left.trait_name.cmp(&right.trait_name)));
        self.grants.dedup();
        self.types_granted
            .sort_by(|left, right| left.named.cmp(&right.named));
        self.types_granted.dedup();
        self.layouts
            .sort_by(|left, right| left.named.cmp(&right.named));
        self.layouts.dedup();
        self.modules
            .sort_by(|left, right| left.names.cmp(&right.names));
        self.modules.dedup();
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
            receiver: None,
            receiver_path: None,
            caller_decides: false,
            reaches:  None,
            takes:    None
        }
    }

    fn resolution(calls: Vec<ResolvedCall>) -> Resolution {
        let mut built = Resolution {
            resolver: "test".to_owned(),
            calls,
            ..Resolution::default()
        };
        built.settle();
        built
    }

    #[test]
    fn a_settled_call_is_found_by_its_location() {
        let resolution = resolution(vec![call("src/lib.rs", 10, "is_some")]);
        let found = resolution
            .at(&located("src/lib.rs", 10))
            .expect("the call is settled");
        assert_eq!(found.callee, "is_some");
        assert_eq!(found.returns, "bool");
    }

    #[test]
    fn a_position_the_resolver_did_not_settle_is_absent() {
        let resolution = resolution(vec![call("src/lib.rs", 10, "go")]);
        assert!(resolution.at(&located("src/lib.rs", 99)).is_none());
    }

    #[test]
    fn settling_orders_by_file_then_position_and_drops_repeats() {
        let resolution = resolution(vec![
            call("src/z.rs", 1, "z"),
            call("src/a.rs", 20, "b"),
            call("src/a.rs", 5, "a"),
            call("src/a.rs", 5, "a"),
        ]);
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

    #[test]
    fn a_type_is_found_at_the_position_the_expression_is_written_at() {
        let mut settled = Resolution {
            resolver: "test".to_owned(),
            types: vec![
                ResolvedType {
                    at:       located("src/lib.rs", 40),
                    rendered: "u32".to_owned(),
                    declared: None
                },
                ResolvedType {
                    at:       located("src/lib.rs", 10),
                    rendered: "String".to_owned(),
                    declared: None
                },
            ],
            ..Resolution::default()
        };
        settled.settle();
        assert_eq!(
            settled
                .type_at(&located("src/lib.rs", 10))
                .expect("the type is settled")
                .rendered,
            "String"
        );
        assert!(settled.type_at(&located("src/lib.rs", 11)).is_none());
    }

    #[test]
    fn two_namesakes_keep_the_crate_that_declares_them_apart() {
        let mut settled = Resolution {
            resolver: "test".to_owned(),
            paths: vec![
                ResolvedPath {
                    at:        located("src/lib.rs", 10),
                    names:     "std::io::Error".to_owned(),
                    kind:      "struct".to_owned(),
                    declaring: "std".to_owned()
                },
                ResolvedPath {
                    at:        located("src/lib.rs", 30),
                    names:     "mycrate::Error".to_owned(),
                    kind:      "enum".to_owned(),
                    declaring: "mycrate".to_owned()
                },
            ],
            ..Resolution::default()
        };
        settled.settle();
        let first = settled
            .path_at(&located("src/lib.rs", 10))
            .expect("the path is settled");
        let second = settled
            .path_at(&located("src/lib.rs", 30))
            .expect("the path is settled");
        assert_ne!(first.declaring, second.declaring);
        assert_ne!(first.names, second.names);
    }

    #[test]
    fn rows_absent_from_a_run_are_absent_from_the_json() {
        let text = serde_json::to_string(&resolution(Vec::new())).unwrap();
        assert!(!text.contains("types"));
        assert!(!text.contains("paths"));
        assert!(!text.contains("fields"));
    }
}
