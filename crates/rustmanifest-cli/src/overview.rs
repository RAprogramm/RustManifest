// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `overview` subcommand: everything the engine knows, in one report.
//!
//! Discovers Rust sources exactly like `check` and prints a deterministic
//! Markdown report of the project as the engine understands it: size and
//! coverage honesty, the module map with cycles, the most connected
//! types, the behavioral role distribution, the outside-world boundary
//! functions, the most contended state fields, and the conventions the
//! learning layer extracted. Findings stay with `check`; this command is
//! the knowledge surface.

use std::{collections::BTreeMap, io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::{
    Effect, FunctionFacts, TypeGraph, behavior, coverage, module_overview, role
};
use rustmanifest_learn::{LearnOptions, learn_roles};

use crate::EXIT_CLEAN;

/// Arguments for the `overview` subcommand.
#[derive(Debug, Parser)]
pub struct OverviewArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Cap for every ranked list in the report.
    #[arg(long, default_value_t = 10)]
    pub top: usize
}

/// Effects that mark a function as an outside-world boundary.
const BOUNDARY_EFFECTS: [Effect; 5] = [
    Effect::FileSystem,
    Effect::Network,
    Effect::Process,
    Effect::Environment,
    Effect::ConsoleIo
];

/// Runs the `overview` subcommand: analyzes the paths and prints the
/// aggregated knowledge report.
pub(crate) fn run_overview(args: &OverviewArgs) -> anyhow::Result<ExitCode> {
    let files =
        rustmanifest_engine::walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let report = render(&sources, args.top);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write!(handle, "{report}").context("writing overview output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Renders the whole report for the analyzed sources.
fn render(sources: &[rustmanifest_engine::Source], top: usize) -> String {
    let mut out = String::new();
    let graph = TypeGraph::from_sources(sources);
    let modules = module_overview(sources);
    let stats = coverage(sources);
    let facts = behavior(sources);
    out.push_str("# Project overview\n\n");
    size_section(&mut out, &stats, facts.len());
    module_section(&mut out, &modules);
    type_section(&mut out, &graph, top);
    role_section(&mut out, &facts);
    boundary_section(&mut out, &facts, top);
    state_section(&mut out, &facts, top);
    convention_section(&mut out, sources, top);
    out.push_str("\nFindings are a separate concern: run `rustmanifest check` for them.\n");
    out
}

/// Prints size and honesty numbers.
fn size_section(out: &mut String, stats: &rustmanifest_engine::Coverage, functions: usize) {
    push(out, "## Size and understanding\n\n");
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
            "- imports resolved: {:.1}% ({} of {})\n\n",
            stats.resolution_rate(),
            stats.use_targets_resolved,
            stats.use_targets_total
        )
    );
}

/// Prints the module map and any dependency cycles.
fn module_section(out: &mut String, modules: &rustmanifest_engine::ModuleOverview) {
    push(out, "## Modules\n\n");
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
        push(out, "- no module dependency cycles\n\n");
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
    push(out, "## Central types\n\n");
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
        push(out, "- no aggregation or interaction edges resolved\n\n");
        return;
    }
    for (name, connections) in connectivity.iter().take(top) {
        push(out, &format!("- `{name}`: {connections} connections\n"));
    }
    truncation_note(out, connectivity.len(), top);
    push(out, "\n");
}

/// Prints the behavioral role distribution.
fn role_section(out: &mut String, facts: &[FunctionFacts]) {
    push(out, "## Behavioral roles\n\n");
    let mut distribution: BTreeMap<String, usize> = BTreeMap::new();
    for fact in facts {
        *distribution.entry(role(fact).to_string()).or_insert(0) += 1;
    }
    for (name, count) in &distribution {
        push(out, &format!("- {name}: {count}\n"));
    }
    push(out, "\n");
}

/// Prints functions that touch the world outside the process.
fn boundary_section(out: &mut String, facts: &[FunctionFacts], top: usize) {
    push(out, "## Outside-world boundaries\n\n");
    let boundaries: Vec<String> = facts
        .iter()
        .filter(|fact| {
            fact.effects
                .iter()
                .any(|effect| BOUNDARY_EFFECTS.contains(effect))
        })
        .map(|fact| {
            let effects: Vec<String> = fact
                .effects
                .iter()
                .filter(|effect| BOUNDARY_EFFECTS.contains(effect))
                .map(ToString::to_string)
                .collect();
            format!("- `{}`: {}\n", fact.function, effects.join(", "))
        })
        .collect();
    if boundaries.is_empty() {
        push(out, "- none observed\n\n");
        return;
    }
    for line in boundaries.iter().take(top) {
        push(out, line);
    }
    truncation_note(out, boundaries.len(), top);
    push(out, "\n");
}

/// Prints the most written state fields and their writer counts.
fn state_section(out: &mut String, facts: &[FunctionFacts], top: usize) {
    push(out, "## Most contended state\n\n");
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
        push(out, "- no resolved field writes observed\n\n");
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
    push(out, "\n");
}

/// Prints the conventions the learning layer extracted.
fn convention_section(out: &mut String, sources: &[rustmanifest_engine::Source], top: usize) {
    push(out, "## Learned conventions\n\n");
    let clauses = learn_roles(sources, LearnOptions::default());
    if clauses.is_empty() {
        push(out, "- nothing learned (too few functions)\n");
        return;
    }
    for clause in clauses.iter().take(top) {
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
    truncation_note(out, clauses.len(), top);
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

    use rustmanifest_engine::Source;

    use super::render;

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
        let report = render(&sources(), 10);
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
    }

    #[test]
    fn report_is_deterministic() {
        assert_eq!(render(&sources(), 10), render(&sources(), 10));
    }
}
