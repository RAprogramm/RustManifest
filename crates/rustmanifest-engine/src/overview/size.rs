// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Size and coverage honesty: how much of the source slice the engine
//! parsed, reached and captured, stated as counted facts rather than
//! confidence.

use super::fragment::push;
use crate::semantic::Coverage;

/// Prints size and honesty numbers.
pub(super) fn size_section(out: &mut String, stats: &Coverage, functions: usize) {
    out.push_str("## Size and understanding\n\n");
    push(
        out,
        &format!(
            "- files: {} analyzed, {} parsed ({:.1}%)\n",
            stats.files_total,
            stats.files_parsed,
            stats.parse_rate()
        )
    );
    push(
        out,
        &format!(
            "- modules: {}, files reachable from crate roots: {} ({:.1}%)\n",
            stats.modules_total,
            stats.files_reachable,
            stats.reachability_rate()
        )
    );
    push(
        out,
        &format!(
            "- types: {} structs, {} enums, {} traits; {} impl blocks, {} methods\n",
            stats.structs_captured,
            stats.enums_captured,
            stats.traits_captured,
            stats.impls_captured,
            stats.methods_captured
        )
    );
    push(
        out,
        &format!("- functions with behavioral facts: {functions}\n")
    );
    push(
        out,
        &format!(
            "- imports: {} total — {} resolved in-workspace, {} outside the workspace (see \
             `coverage` for the external/unresolved split)\n\n",
            stats.use_targets_total,
            stats.use_targets_resolved,
            stats.use_targets_total - stats.use_targets_resolved
        )
    );
}
