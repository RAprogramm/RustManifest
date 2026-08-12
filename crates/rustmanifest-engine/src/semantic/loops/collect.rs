// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Candidate collection: walks every function, finds loops, and turns
//! each eligible loop-local `let` into a [`Candidate`] carrying its
//! origin classification and use summary. The rule layer consumes the
//! candidates; this module never decides what is a violation.
//!
//! The walk over sources and items lives here; everything a single loop
//! needs lives beside it: [`candidate`] holds the records handed onward,
//! [`scan`] analyzes one loop body, [`binding`] and [`mutation`] supply the
//! identifier sets the origin classification asks for, and [`hoist`]
//! turns a statement span into fix coordinates.

mod binding;
mod candidate;
mod hoist;
mod mutation;
mod scan;

use std::collections::BTreeMap;

use syn::visit::Visit;

pub(super) use self::candidate::Candidate;
use self::scan::LoopFinder;
use crate::{
    semantic::{behavior::ParamMode, expr::impl_type_name},
    source::Source
};

/// How every settled call of one source takes the value it is written on,
/// by the byte range of the call.
///
/// What a call does to its receiver — leave it, change it, or end its
/// life — is the mode the function it reaches declares. Empty where no
/// resolver ran, and an absent call is one nothing here can answer for.
pub(super) type Taken = BTreeMap<(u32, u32), ParamMode>;

/// The receiver modes a resolver settled for one source.
fn taken(resolution: Option<&rustmanifest_schema::Resolution>, source: &Source) -> Taken {
    let mut settled = Taken::new();
    let Some(resolution) = resolution else {
        return settled;
    };
    for call in &resolution.calls {
        if call.at.file != source.path() {
            continue;
        }
        if let Some(mode) = call.takes {
            settled.insert(
                (call.at.range.start, call.at.range.end),
                match mode {
                    rustmanifest_schema::ResolvedMode::Owned => ParamMode::Owned,
                    rustmanifest_schema::ResolvedMode::Borrowed => ParamMode::Borrowed,
                    rustmanifest_schema::ResolvedMode::BorrowedMut => ParamMode::BorrowedMut
                }
            );
        }
    }
    settled
}

/// Collects loop-local `let` candidates from every function in `sources`.
pub(super) fn candidates(
    sources: &[Source],
    resolution: Option<&rustmanifest_schema::Resolution>
) -> Vec<Candidate> {
    let mut found = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let text = source.text();
        let takes = taken(resolution, source);
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    let mut finder = LoopFinder {
                        source: index,
                        function: function.sig.ident.to_string(),
                        text,
                        takes: &takes,
                        candidates: &mut found
                    };
                    finder.visit_block(&function.block);
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        let mut finder = LoopFinder {
                            source: index,
                            function: format!("{type_name}::{}", method.sig.ident),
                            text,
                            takes: &takes,
                            candidates: &mut found
                        };
                        finder.visit_block(&method.block);
                    }
                }
                _ => {}
            }
        }
    }
    found
}
