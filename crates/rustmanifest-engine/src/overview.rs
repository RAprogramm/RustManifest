// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One-report knowledge surface: everything the engine understands about
//! an analyzed source slice, rendered as deterministic Markdown.
//!
//! Sections: size and coverage honesty, the module map with cycle
//! warnings, the most connected types, the behavioral role distribution,
//! outside-world boundary functions, the most contended state fields, and
//! learned conventions. Each section owns a submodule; this module only
//! gathers the analyses once and hands them out in report order.
//! Convention clauses arrive as plain data so this module needs no
//! knowledge of the learning layer; callers that skip learning pass an
//! empty slice and the section says so.

mod boundaries;
mod capability;
mod central_types;
mod conventions;
mod fragment;
mod keeping;
mod layers;
mod memory_shape;
mod module_map;
mod roles;
mod size;
mod state;
mod surface;

pub use self::conventions::Convention;
use self::{
    boundaries::boundary_section, capability::capability_section, central_types::type_section,
    conventions::convention_section, keeping::keeping_section, layers::layer_section,
    memory_shape::shape_section, module_map::module_section, roles::role_section,
    size::size_section, state::state_section, surface::surface_section
};
use crate::semantic::Workspace;

/// Renders the whole knowledge report for the analyzed sources.
///
/// `top` caps every ranked list; truncation is always announced, never
/// silent. Output is deterministic: same sources, same conventions, same
/// report.
#[must_use]
pub fn overview_markdown(
    workspace: &Workspace<'_>,
    top: usize,
    conventions: &[Convention]
) -> String {
    let mut out = String::new();
    let sources = workspace.sources();
    let graph = workspace.types();
    let modules = crate::semantic::module_overview(workspace);
    let stats = crate::semantic::coverage(workspace);
    let facts = workspace.facts();
    out.push_str("# Project overview\n\n");
    size_section(&mut out, &stats, facts.len());
    module_section(&mut out, &modules);
    type_section(&mut out, graph, top);
    role_section(&mut out, facts);
    surface_section(&mut out, sources, top);
    boundary_section(&mut out, sources, facts, top);
    state_section(&mut out, facts, top);
    shape_section(&mut out, sources, workspace.resolution(), top);
    layer_section(&mut out, workspace, top);
    capability_section(&mut out, workspace, top);
    keeping_section(&mut out, workspace);
    convention_section(&mut out, conventions, top);
    out.push_str("\nFindings are a separate concern: run `rustmanifest check` for them.\n");
    out
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

    use super::{Convention, overview_markdown};
    use crate::{semantic::Workspace, source::Source};

    fn sources() -> [Source; 1] {
        [Source::new(
            PathBuf::from("src/lib.rs"),
            "pub struct Wallet { pub balance: i64 }\nimpl Wallet {\n    pub fn credit(&mut \
             self, amount: i64) { self.balance += amount; }\n    pub fn balance(&self) -> i64 { \
             self.balance }\n}\npub fn audit(wallet: &Wallet) { println!(\"{}\", \
             wallet.balance); }\n"
                .to_owned()
        )]
    }

    #[test]
    fn report_carries_every_section() {
        let conventions = [Convention {
            role:     "io".to_owned(),
            includes: vec!["effect-console".to_owned()],
            excludes: Vec::new()
        }];
        let report = overview_markdown(&Workspace::new(&sources()), 10, &conventions);
        for heading in [
            "## Size and understanding",
            "## Modules",
            "## Central types",
            "## Behavioral roles",
            "## Outside-world boundaries",
            "## Most contended state",
            "## Learned conventions"
        ] {
            assert!(report.contains(heading), "missing {heading}");
        }
        assert!(report.contains("`audit`: console"));
        assert!(report.contains("`Wallet.balance`: 1 writer(s): Wallet::credit"));
        assert!(report.contains("io <= IF `effect-console`"));
    }

    #[test]
    fn report_is_deterministic() {
        assert_eq!(
            overview_markdown(&Workspace::new(&sources()), 10, &[]),
            overview_markdown(&Workspace::new(&sources()), 10, &[])
        );
    }
}
