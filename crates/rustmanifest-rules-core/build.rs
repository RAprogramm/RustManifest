// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Build script that bundles every rule defined under `rules/` into a single
//! `rules.json` artifact emitted in `OUT_DIR`. The runtime crate
//! `include_str!`s the result and parses it once on first access through a
//! `LazyLock`.

use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf}
};

use rustmanifest_schema::Rule;

type BuildError = Box<dyn std::error::Error>;

fn main() -> Result<(), BuildError> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let rules_dir = manifest_dir.join("rules");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    emit_rerun(&rules_dir)?;

    let rules = collect_rules(&rules_dir)?;
    let json = serde_json::to_string_pretty(&rules)?;
    fs::write(out_dir.join("rules.json"), json)?;

    Ok(())
}

fn collect_rules(rules_dir: &Path) -> Result<Vec<Rule>, BuildError> {
    let mut entries: Vec<_> = fs::read_dir(rules_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect();
    entries.sort_by_key(std::fs::DirEntry::path);

    let mut rules = Vec::with_capacity(entries.len());
    for entry in entries {
        rules.push(parse_one(&entry.path())?);
    }
    Ok(rules)
}

fn parse_one(dir: &Path) -> Result<Rule, BuildError> {
    let dir_name_os = dir
        .file_name()
        .ok_or_else(|| format!("cannot read file name of {}", dir.display()))?;
    let dir_name = dir_name_os
        .to_str()
        .ok_or_else(|| format!("non-utf8 directory name at {}", dir.display()))?;

    let rule_toml = dir.join("rule.toml");
    let pass_rs = dir.join("pass.rs");
    let fail_rs = dir.join("fail.rs");

    if !rule_toml.is_file() {
        return Err(format!("missing rule.toml in {}", dir.display()).into());
    }
    if !pass_rs.is_file() {
        return Err(format!("missing pass.rs in {}", dir.display()).into());
    }
    if !fail_rs.is_file() {
        return Err(format!("missing fail.rs in {}", dir.display()).into());
    }

    let text = fs::read_to_string(&rule_toml)?;
    let rule: Rule = toml::from_str(&text)?;

    if rule.id != dir_name {
        return Err(format!(
            "rule id {:?} does not match directory name {:?} in {}",
            rule.id,
            dir_name,
            dir.display()
        )
        .into());
    }

    Ok(rule)
}

fn emit_rerun(rules_dir: &Path) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "cargo::rerun-if-changed={}", rules_dir.display())?;
    writeln!(stdout, "cargo::rerun-if-changed=build.rs")?;
    Ok(())
}
