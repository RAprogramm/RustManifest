// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Memory shape: what copying a value provably costs and how many heap
//! indirections it drags, ranked heaviest first.

use super::fragment::{push, truncation_note};
use crate::source::Source;

/// Prints the heaviest types by guaranteed field payload: what a copy
/// provably costs and how many heap indirections each value drags.
pub(super) fn shape_section(out: &mut String, sources: &[Source], top: usize) {
    out.push_str("## Memory shape\n\n");
    let mut weighed: Vec<crate::semantic::TypeShape> = crate::semantic::shapes(sources)
        .into_iter()
        .filter(|shape| shape.payload.is_some() || shape.indirections > 0)
        .collect();
    if weighed.is_empty() {
        out.push_str("- no sizable types observed\n\n");
        return;
    }
    weighed.sort_by(|a, b| {
        b.payload
            .unwrap_or(0)
            .cmp(&a.payload.unwrap_or(0))
            .then_with(|| b.indirections.cmp(&a.indirections))
            .then_with(|| a.name.cmp(&b.name))
    });
    for shape in weighed.iter().take(top) {
        let payload = shape.payload.map_or_else(
            || "payload unresolved".to_owned(),
            |bytes| format!("{bytes} B guaranteed field payload")
        );
        let copy = if shape.copyable { ", Copy" } else { "" };
        push(
            out,
            &format!(
                "- `{}`: {payload}, {} heap indirection(s){copy}\n",
                shape.name, shape.indirections
            )
        );
    }
    truncation_note(out, weighed.len(), top);
    out.push('\n');
}
