// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Schema export tool.
//!
//! Generates deterministic JSON Schema files for every canonical type defined
//! in [`rustmanifest_schema`] and writes them to a target directory. CI invokes
//! this binary on every PR and fails on drift against the committed schemas
//! under `crates/rustmanifest-schema/schemas/`.

use std::{
    fs,
    path::{Path, PathBuf}
};

use clap::Parser;
use rustmanifest_schema::{Config, Finding, MethodologyResource, Rule};
use schemars::{Schema, schema_for};

/// Command-line arguments for the schema export tool.
#[derive(Debug, Parser)]
#[command(name = "rustmanifest-schema-export", version, about)]
struct Args {
    /// Output directory for the generated `*.schema.json` files.
    #[arg(long)]
    out: PathBuf
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    run(&args.out)
}

fn run(out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(out)?;
    write_schema(out, "rule", &schema_for!(Rule))?;
    write_schema(out, "finding", &schema_for!(Finding))?;
    write_schema(out, "config", &schema_for!(Config))?;
    write_schema(
        out,
        "methodology-resource",
        &schema_for!(MethodologyResource)
    )?;
    Ok(())
}

fn write_schema(
    out: &Path,
    name: &str,
    schema: &Schema
) -> Result<(), Box<dyn std::error::Error>> {
    let path = out.join(format!("{name}.schema.json"));
    let json = serde_json::to_string_pretty(schema)?;
    fs::write(path, json)?;
    Ok(())
}
