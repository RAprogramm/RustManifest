// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The inferred stratification: modules grouped by dependency depth, with
//! mutually calling modules named as entangled instead of forced into a
//! layer they do not have.

use std::collections::BTreeMap;

use super::fragment::push;
use crate::semantic::Workspace;

/// Prints the inferred module stratification.
pub(super) fn layer_section(out: &mut String, workspace: &Workspace<'_>, top: usize) {
    push(out, "## Inferred layers\n\n");
    let all = crate::semantic::layers(workspace);
    let mut by_depth: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    let mut entangled: Vec<&str> = Vec::new();
    for layer in &all {
        match layer.depth {
            Some(depth) => by_depth.entry(depth).or_default().push(&layer.module),
            None => entangled.push(&layer.module)
        }
    }
    if by_depth.is_empty() && entangled.is_empty() {
        push(out, "- no module owns any function\n\n");
        return;
    }
    for (depth, modules) in by_depth.iter().rev() {
        let shown: Vec<&str> = modules.iter().copied().take(top).collect();
        let label = if *depth == 0 { " (foundation)" } else { "" };
        push(
            out,
            &format!("- depth {depth}{label}: {}", shown.join(", "))
        );
        if modules.len() > top {
            push(out, &format!(" ... and {} more", modules.len() - top));
        }
        push(out, "\n");
    }
    if !entangled.is_empty() {
        push(
            out,
            &format!("- entangled (mutual calls): {}\n", entangled.join(", "))
        );
    }
    push(out, "\n");
}
