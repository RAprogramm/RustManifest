// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One-report knowledge surface: everything the engine understands about
//! an analyzed source slice, rendered as deterministic Markdown.
//!
//! Sections: size and coverage honesty, the module map with cycle
//! warnings, the most connected types, the behavioral role distribution,
//! outside-world boundary functions, the most contended state fields, and
//! learned conventions. Convention clauses arrive as plain data so this
//! module needs no knowledge of the learning layer; callers that skip
//! learning pass an empty slice and the section says so.

use std::collections::BTreeMap;

use crate::{
    semantic::{Coverage, Effect, FunctionFacts, ModuleOverview, TypeGraph},
    source::Source
};

/// One learned convention clause, as plain renderable data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convention {
    /// Role the clause votes for.
    pub role:     String,
    /// Feature names that must be present.
    pub includes: Vec<String>,
    /// Feature names that must be absent.
    pub excludes: Vec<String>
}

/// Effects that mark a function as an outside-world boundary.
const BOUNDARY_EFFECTS: [Effect; 5] = [
    Effect::FileSystem,
    Effect::Network,
    Effect::Process,
    Effect::Environment,
    Effect::ConsoleIo
];

/// Renders the whole knowledge report for the analyzed sources.
///
/// `top` caps every ranked list; truncation is always announced, never
/// silent. Output is deterministic: same sources, same conventions, same
/// report.
#[must_use]
pub fn overview_markdown(sources: &[Source], top: usize, conventions: &[Convention]) -> String {
    let mut out = String::new();
    let graph = TypeGraph::from_sources(sources);
    let modules = crate::semantic::module_overview(sources);
    let stats = crate::semantic::coverage(sources);
    let facts = crate::semantic::behavior(sources);
    out.push_str("# Project overview\n\n");
    size_section(&mut out, &stats, facts.len());
    module_section(&mut out, &modules);
    type_section(&mut out, &graph, top);
    role_section(&mut out, &facts);
    surface_section(&mut out, sources, top);
    boundary_section(&mut out, sources, &facts, top);
    state_section(&mut out, &facts, top);
    shape_section(&mut out, sources, top);
    layer_section(&mut out, sources, top);
    convention_section(&mut out, conventions, top);
    out.push_str("\nFindings are a separate concern: run `rustmanifest check` for them.\n");
    out
}

/// Prints size and honesty numbers.
fn size_section(out: &mut String, stats: &Coverage, functions: usize) {
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

/// Prints the module map and any dependency cycles.
fn module_section(out: &mut String, modules: &ModuleOverview) {
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

/// Prints the most connected types.
fn type_section(out: &mut String, graph: &TypeGraph, top: usize) {
    out.push_str("## Central types\n\n");
    let mut connectivity: Vec<(String, usize)> = graph
        .types()
        .iter()
        .map(|node| {
            let inbound = graph.aggregators_of(&node.name).len();
            let outbound = graph
                .aggregation_edges()
                .iter()
                .filter(|(owner, _, _)| owner.name == node.name)
                .count();
            let interactions = graph
                .interaction_edges()
                .iter()
                .filter(|(_, target)| target.name == node.name)
                .count();
            (node.name.clone(), inbound + outbound + interactions)
        })
        .filter(|(_, connections)| *connections > 0)
        .collect();
    connectivity.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if connectivity.is_empty() {
        out.push_str("- no aggregation or interaction edges resolved\n\n");
        return;
    }
    for (name, connections) in connectivity.iter().take(top) {
        push(out, &format!("- `{name}`: {connections} connections\n"));
    }
    truncation_note(out, connectivity.len(), top);
    out.push('\n');
}

/// Prints the behavioral role distribution.
fn role_section(out: &mut String, facts: &[FunctionFacts]) {
    out.push_str("## Behavioral roles\n\n");
    let mut distribution: BTreeMap<String, usize> = BTreeMap::new();
    for fact in facts {
        *distribution
            .entry(crate::semantic::role(fact).to_string())
            .or_insert(0) += 1;
    }
    for (name, count) in &distribution {
        push(out, &format!("- {name}: {count}\n"));
    }
    out.push('\n');
}

/// Prints the external contract recovered from framework registration
/// calls: method, path, and handler per endpoint, with the honesty
/// counters that state what could not be recovered.
fn surface_section(out: &mut String, sources: &[Source], top: usize) {
    let recovered = crate::semantic::surface(sources);
    if recovered.routes.is_empty()
        && recovered.dynamic == 0
        && recovered.unresolved.is_empty()
        && recovered.entangled.is_empty()
    {
        return;
    }
    out.push_str("## External surface\n\n");
    push(
        out,
        &format!(
            "- {} endpoint(s) from {} router-building function(s)\n",
            recovered.routes.len(),
            recovered.providers
        )
    );
    if recovered.conditional > 0 {
        push(
            out,
            &format!(
                "- {} registration(s) standing under a branch, arm, loop or closure — present \
                 only where that construct runs\n",
                recovered.conditional
            )
        );
    }
    if recovered.dynamic > 0 {
        push(
            out,
            &format!(
                "- {} registration(s) with a path assembled at runtime — not \
                 recoverable\n",
                recovered.dynamic
            )
        );
    }
    if !recovered.unresolved.is_empty() {
        push(
            out,
            &format!(
                "- {} composition(s) naming no resolvable router function\n",
                recovered.unresolved.len()
            )
        );
    }
    if !recovered.entangled.is_empty() {
        push(
            out,
            &format!(
                "- router functions folding each other, composition order undetermined: \
                 {}\n",
                recovered.entangled.join(", ")
            )
        );
    }
    for route in recovered.routes.iter().take(top) {
        let file = sources
            .get(route.source)
            .map_or_else(String::new, |source| format!(" ({})", source.short_path()));
        push(
            out,
            &format!(
                "- `{} {}` → `{}`{}{file}\n",
                route.method,
                route.path,
                route.handler,
                if route.conditional {
                    " (conditional)"
                } else {
                    ""
                }
            )
        );
    }
    if recovered.routes.len() > top {
        push(
            out,
            &format!("- ... {} more endpoint(s)\n", recovered.routes.len() - top)
        );
    }
    out.push('\n');
}

/// Prints functions that touch the world outside the process; a name
/// shared by several functions is qualified by its file.
fn boundary_section(out: &mut String, sources: &[Source], facts: &[FunctionFacts], top: usize) {
    out.push_str("## Outside-world boundaries\n\n");
    let touching: Vec<&FunctionFacts> = facts
        .iter()
        .filter(|fact| {
            fact.effects
                .iter()
                .any(|effect| BOUNDARY_EFFECTS.contains(effect))
        })
        .collect();
    let boundaries: Vec<String> = touching
        .iter()
        .map(|fact| {
            let effects: Vec<String> = fact
                .effects
                .iter()
                .filter(|effect| BOUNDARY_EFFECTS.contains(effect))
                .map(ToString::to_string)
                .collect();
            let duplicated = touching
                .iter()
                .filter(|other| other.function == fact.function)
                .count()
                > 1;
            let qualifier = if duplicated {
                sources
                    .get(fact.source)
                    .map_or_else(String::new, |source| format!(" ({})", source.short_path()))
            } else {
                String::new()
            };
            format!("- `{}`{qualifier}: {}\n", fact.function, effects.join(", "))
        })
        .collect();
    if boundaries.is_empty() {
        out.push_str("- none observed\n\n");
        return;
    }
    for line in boundaries.iter().take(top) {
        out.push_str(line);
    }
    truncation_note(out, boundaries.len(), top);
    out.push('\n');
}

/// Prints the heaviest types by guaranteed field payload: what a copy
/// provably costs and how many heap indirections each value drags.
fn shape_section(out: &mut String, sources: &[Source], top: usize) {
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

/// Prints the most written state fields and their writer counts.
fn state_section(out: &mut String, facts: &[FunctionFacts], top: usize) {
    out.push_str("## Most contended state\n\n");
    let mut writers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for fact in facts {
        for write in &fact.writes {
            if !write.starts_with("?.") {
                writers.entry(write).or_default().push(&fact.function);
            }
        }
    }
    let mut ranked: Vec<(&str, Vec<&str>)> = writers.into_iter().collect();
    ranked.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
    if ranked.is_empty() {
        out.push_str("- no resolved field writes observed\n\n");
        return;
    }
    for (field, mut names) in ranked.iter().take(top).map(|(f, n)| (*f, n.clone())) {
        names.sort_unstable();
        names.dedup();
        push(
            out,
            &format!(
                "- `{field}`: {} writer(s): {}\n",
                names.len(),
                names.join(", ")
            )
        );
    }
    truncation_note(out, ranked.len(), top);
    out.push('\n');
}

/// Prints the inferred module stratification.
fn layer_section(out: &mut String, sources: &[Source], top: usize) {
    push(out, "## Inferred layers\n\n");
    let all = crate::semantic::layers(sources);
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

/// Prints the conventions the learning layer extracted.
fn convention_section(out: &mut String, conventions: &[Convention], top: usize) {
    out.push_str("## Learned conventions\n\n");
    if conventions.is_empty() {
        out.push_str("- nothing learned (too few functions)\n");
        return;
    }
    for clause in conventions.iter().take(top) {
        let mut parts: Vec<String> = clause
            .includes
            .iter()
            .map(|name| format!("`{name}`"))
            .collect();
        parts.extend(clause.excludes.iter().map(|name| format!("NOT `{name}`")));
        push(
            out,
            &format!("- {} <= IF {}\n", clause.role, parts.join(" AND "))
        );
    }
    truncation_note(out, conventions.len(), top);
}

/// Appends a note when a ranked list was truncated.
fn truncation_note(out: &mut String, total: usize, top: usize) {
    if total > top {
        push(out, &format!("- ... and {} more\n", total - top));
    }
}

/// Appends a fragment to the report.
fn push(out: &mut String, fragment: &str) {
    out.push_str(fragment);
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
    use crate::source::Source;

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
        let report = overview_markdown(&sources(), 10, &conventions);
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
            overview_markdown(&sources(), 10, &[]),
            overview_markdown(&sources(), 10, &[])
        );
    }
}
