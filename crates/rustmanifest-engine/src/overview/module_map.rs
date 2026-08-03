// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The module map: how many modules sit under how many crate roots, how
//! many dependency edges join them, and which of them stand in a cycle.

use super::fragment::push;
use crate::semantic::ModuleOverview;

/// Prints the module map and any dependency cycles.
pub(super) fn module_section(out: &mut String, modules: &ModuleOverview) {
    out.push_str("## Modules\n\n");
    let roots = modules
        .modules()
        .iter()
        .filter(|module| module.crate_root)
        .count();
    push(
        out,
        &format!(
            "- {} modules across {} crate roots, {} dependency edges\n",
            modules.modules().len(),
            roots,
            modules.edges().len()
        )
    );
    let cycling: Vec<&str> = modules
        .modules()
        .iter()
        .filter(|module| module.in_cycle)
        .map(|module| module.path.as_str())
        .collect();
    if cycling.is_empty() {
        out.push_str("- no module dependency cycles\n\n");
    } else {
        push(
            out,
            &format!(
                "- CYCLE WARNING, modules involved: {}\n\n",
                cycling.join(", ")
            )
        );
    }
}
