// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line surface of the `graph` subcommand: the analyzed paths, the
//! diagram content selector and the diagram output format.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

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
    pub format: DiagramFormat,

    /// Resolution overlay to read the module tree from. A module diagram
    /// needs one: which file holds which module is the compiler's answer.
    #[arg(long)]
    pub resolution: Option<PathBuf>,
}

/// Diagram content selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GraphKind {
    /// One node per module, one edge per resolved intra-crate dependency.
    Modules,
    /// One node per structure, one edge per aggregating field.
    Types,
}

/// Diagram output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiagramFormat {
    /// Mermaid `graph LR` flowchart.
    Mermaid,
    /// Graphviz DOT digraph.
    Dot,
}
