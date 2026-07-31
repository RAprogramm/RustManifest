// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `graph` subcommand: architecture-diagram export.
//!
//! Discovers Rust sources exactly like `check`, builds the requested view —
//! module dependencies via [`module_overview`] or type aggregation via
//! [`TypeGraph`] — and prints a Mermaid or Graphviz DOT diagram to stdout.
//! Output is deterministic: nodes and edges are sorted and deduplicated.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::PathBuf,
    process::ExitCode
};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use rustmanifest_engine::{ModuleOverview, TypeGraph, module_overview, walker};

use crate::EXIT_CLEAN;

/// Arguments for the `graph` subcommand.
#[derive(Debug, Parser)]
pub struct GraphArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Diagram content: module dependencies or type aggregation.
    #[arg(long, value_enum, default_value_t = GraphKind::Modules)]
    pub kind: GraphKind,

    /// Diagram output format.
    #[arg(long, value_enum, default_value_t = DiagramFormat::Mermaid)]
    pub format: DiagramFormat
}

/// Diagram content selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GraphKind {
    /// One node per module, one edge per resolved intra-crate dependency.
    Modules,
    /// One node per structure, one edge per aggregating field.
    Types
}

/// Diagram output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiagramFormat {
    /// Mermaid `graph LR` flowchart.
    Mermaid,
    /// Graphviz DOT digraph.
    Dot
}

/// Runs the `graph` subcommand: discovers files, reads sources within the
/// default per-file memory budget, and prints the requested diagram.
pub(crate) fn run_graph(args: &GraphArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let lines = match args.kind {
        GraphKind::Modules => module_lines(&module_overview(&sources), args.format),
        GraphKind::Types => type_lines(&TypeGraph::from_sources(&sources), args.format)
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for line in lines {
        writeln!(handle, "{line}").context("writing diagram output")?;
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Renders the module dependency diagram, one output line per element.
///
/// Nodes are sorted by module path and merged across crates; edges are
/// sorted by `(from, to)` and deduplicated. Modules participating in a
/// dependency cycle are styled: Mermaid nodes get the `cycle` class, DOT
/// nodes are drawn red.
fn module_lines(overview: &ModuleOverview, format: DiagramFormat) -> Vec<String> {
    let mut nodes: BTreeMap<&str, bool> = BTreeMap::new();
    for module in overview.modules() {
        let in_cycle = nodes.entry(module.path.as_str()).or_insert(false);
        *in_cycle = *in_cycle || module.in_cycle;
    }
    let edges: BTreeSet<(&str, &str)> = overview
        .edges()
        .iter()
        .filter(|edge| edge.from != edge.to)
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    match format {
        DiagramFormat::Mermaid => {
            let mut lines = vec!["graph LR".to_owned()];
            if nodes.values().any(|&in_cycle| in_cycle) {
                lines.push("    classDef cycle stroke:#cc0000,stroke-width:2px".to_owned());
            }
            for (path, in_cycle) in &nodes {
                let id = mermaid_id(path);
                let class = if *in_cycle { ":::cycle" } else { "" };
                lines.push(format!("    {id}[\"{path}\"]{class}"));
            }
            for (from, to) in &edges {
                lines.push(format!("    {} --> {}", mermaid_id(from), mermaid_id(to)));
            }
            lines
        }
        DiagramFormat::Dot => {
            let mut lines = vec!["digraph modules {".to_owned(), "    rankdir=LR;".to_owned()];
            for (path, in_cycle) in &nodes {
                let attributes = if *in_cycle { " [color=\"red\"]" } else { "" };
                lines.push(format!("    \"{path}\"{attributes};"));
            }
            for (from, to) in &edges {
                lines.push(format!("    \"{from}\" -> \"{to}\";"));
            }
            lines.push("}".to_owned());
            lines
        }
    }
}

/// Renders the type diagram, one output line per element: solid arrows for
/// aggregation (labeled with the field), dashed arrows for interaction (a
/// method body of the source type calling into the target type).
///
/// Nodes are sorted by type name and deduplicated; edges are sorted by
/// `(owner, label, target)`.
fn type_lines(graph: &TypeGraph, format: DiagramFormat) -> Vec<String> {
    let nodes: BTreeSet<&str> = graph
        .types()
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    let edges: BTreeSet<(&str, &str, &str)> = graph
        .aggregation_edges()
        .into_iter()
        .map(|(owner, field, target)| {
            (
                owner.name.as_str(),
                field.name.as_str(),
                target.name.as_str()
            )
        })
        .collect();
    let interactions: BTreeSet<(&str, &str)> = graph
        .interaction_edges()
        .into_iter()
        .map(|(impl_block, target)| (impl_block.type_name.as_str(), target.name.as_str()))
        .filter(|(from, to)| {
            !edges
                .iter()
                .any(|(owner, _, target)| owner == from && target == to)
        })
        .collect();
    match format {
        DiagramFormat::Mermaid => {
            let mut lines = vec!["graph LR".to_owned()];
            for name in &nodes {
                lines.push(format!("    {name}[\"{name}\"]"));
            }
            for (owner, field, target) in &edges {
                lines.push(format!("    {owner} -->|{field}| {target}"));
            }
            for (from, to) in &interactions {
                lines.push(format!("    {from} -.->|calls| {to}"));
            }
            lines
        }
        DiagramFormat::Dot => {
            let mut lines = vec!["digraph types {".to_owned(), "    rankdir=LR;".to_owned()];
            for name in &nodes {
                lines.push(format!("    \"{name}\";"));
            }
            for (owner, field, target) in &edges {
                lines.push(format!(
                    "    \"{owner}\" -> \"{target}\" [label=\"{field}\"];"
                ));
            }
            for (from, to) in &interactions {
                lines.push(format!(
                    "    \"{from}\" -> \"{to}\" [style=\"dashed\" label=\"calls\"];"
                ));
            }
            lines.push("}".to_owned());
            lines
        }
    }
}

/// Sanitizes a module path into a Mermaid node identifier.
fn mermaid_id(path: &str) -> String {
    path.replace("::", "_")
}
