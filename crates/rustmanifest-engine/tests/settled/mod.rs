// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the compiler settles about the rule fixtures, asked once for all
//! of them.
//!
//! A fixture is a loose file, and a resolver reads a cargo workspace, so
//! every fixture is mounted as a module of one throwaway crate and the
//! whole crate is resolved in a single load. The file each fixture is
//! copied to holds its bytes unchanged, so every range the resolver
//! states is a range in the fixture itself and only the path has to be
//! put back. A resolver states that path relative to the crate root it
//! read, so that is the form the mounting records and the form the rows
//! are looked up under.
//!
//! This is what lets the fixture suite judge a rule against the
//! compiler's own answers instead of an overlay written by hand beside
//! the assertions.

#![allow(
    dead_code,
    reason = "each integration test binary compiles this module and uses the part it needs"
)]

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf}
};

use rustmanifest_engine::Source;
use rustmanifest_schema::Resolution;

/// One fixture side to resolve: the name it is asked back under, and its
/// sources.
pub struct Case {
    /// Name the resolution is handed back under.
    pub name:    String,
    /// Sources of this side, as the harness reads them.
    pub sources: Vec<Source>
}

/// Resolves every case in one load, answering the resolution of each by
/// its name.
///
/// A case whose sources reach no row of the resolution answers with an
/// empty resolution rather than being absent: the compiler was asked and
/// said nothing about it, which is a different thing from not asking.
///
/// # Panics
///
/// Panics when the throwaway workspace cannot be written or the resolver
/// fails to read it: a harness that cannot ask the compiler has nothing
/// to judge a rule against, and a silent pass would be the lie.
#[must_use]
pub fn settled(cases: &[Case]) -> BTreeMap<String, Resolution> {
    let bed = tempfile::tempdir().expect("a temporary directory");
    let root = bed.path();
    let mut members = String::new();
    let mut origins: BTreeMap<PathBuf, (String, PathBuf)> = BTreeMap::new();
    for (index, case) in cases.iter().enumerate() {
        let member = format!("case{index}");
        writeln!(members, "  \"{member}\",").expect("a string takes what is written to it");
        mount(root, &member, case, &mut origins);
    }
    fs::write(
        root.join("Cargo.toml"),
        format!("[workspace]\nresolver = \"3\"\nmembers = [\n{members}]\n")
    )
    .expect("writing the workspace manifest");
    let resolution = rustmanifest_resolve::resolve(root).expect("the fixture workspace resolves");
    partition(&resolution, &origins, cases)
}

/// Writes one case as a crate of its own, recording where each file
/// landed.
///
/// A crate rather than a module, because a fixture writing `crate::` means
/// itself: mounted under a shared root, every such path would reach for a
/// name that root does not declare, and the reading would refuse where the
/// fixture is whole.
fn mount(
    root: &Path,
    module: &str,
    case: &Case,
    origins: &mut BTreeMap<PathBuf, (String, PathBuf)>
) {
    fs::create_dir_all(root.join(module).join("src")).expect("the crate source directory");
    fs::write(
        root.join(module).join("Cargo.toml"),
        format!(
            "[package]\nname = \"{module}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\n"
        )
    )
    .expect("writing a member manifest");
    let anchor = case
        .sources
        .iter()
        .position(|source| {
            source
                .path()
                .file_stem()
                .and_then(|held| held.to_str())
                .is_some_and(|stem| matches!(stem, "lib" | "main"))
        })
        .unwrap_or(0);
    for (index, source) in case.sources.iter().enumerate() {
        let landed = if index == anchor {
            root.join(module).join("src").join("lib.rs")
        } else {
            let stem = source
                .path()
                .file_stem()
                .and_then(|held| held.to_str())
                .unwrap_or("held");
            root.join(module).join("src").join(format!("{stem}.rs"))
        };
        if let Some(parent) = landed.parent() {
            fs::create_dir_all(parent).expect("the module directory");
        }
        fs::write(&landed, source.text()).expect("writing a fixture module");
        let stated = landed
            .strip_prefix(root)
            .expect("the file was written under the crate root")
            .to_path_buf();
        origins.insert(stated, (case.name.clone(), source.path().to_path_buf()));
    }
}

/// Splits one resolution into the resolution of each case, with every row
/// put back on the fixture path it came from.
fn partition(
    resolution: &Resolution,
    origins: &BTreeMap<PathBuf, (String, PathBuf)>,
    cases: &[Case]
) -> BTreeMap<String, Resolution> {
    let mut split: BTreeMap<String, Resolution> = cases
        .iter()
        .map(|case| {
            (case.name.clone(), Resolution {
                resolver: "compiler".to_owned(),
                ..Resolution::default()
            })
        })
        .collect();
    macro_rules! carry {
        ($field:ident) => {
            for row in &resolution.$field {
                let Some((name, origin)) = origins.get(&row.at.file) else {
                    continue;
                };
                let Some(held) = split.get_mut(name) else {
                    continue;
                };
                let mut row = row.clone();
                row.at.file = origin.clone();
                held.$field.push(row);
            }
        };
    }
    carry!(calls);
    carry!(fields);
    carry!(types);
    carry!(paths);
    carry!(functions);
    carry!(signatures);
    for held in split.values_mut() {
        for path in &mut held.paths {
            path.names = renamed(&path.names);
            path.declaring = renamed(&path.declaring);
        }
    }
    carry_modules(resolution, origins, &mut split);
    for held in split.values_mut() {
        held.types_granted.clone_from(&resolution.types_granted);
        held.layouts.clone_from(&resolution.layouts);
        held.settle();
    }
    split
}

/// One name with the throwaway crate's own segment replaced by the name a
/// fixture carries when it is read alone.
///
/// Module names and the names a path reaches both begin with the crate,
/// and a rule reading one against the other would compare a renamed name
/// with an unrenamed one.
fn renamed(names: &str) -> String {
    names.split_once("::").map_or_else(
        || "fixture".to_owned(),
        |(_, below)| format!("fixture::{below}")
    )
}

/// Puts the module tree back the way a fixture stands on its own.
///
/// Every case is a crate of the throwaway workspace, so what the resolver
/// states is `caseN` and `caseN::held`. The readers above name a fixture's
/// own root, so the crate is renamed to it and everything below keeps its
/// place beneath.
fn carry_modules(
    resolution: &Resolution,
    origins: &BTreeMap<PathBuf, (String, PathBuf)>,
    split: &mut BTreeMap<String, Resolution>
) {
    for module in &resolution.modules {
        let Some((name, origin)) = origins.get(&module.file) else {
            continue;
        };
        let Some(held) = split.get_mut(name) else {
            continue;
        };
        let root = !module.names.contains("::");
        let names = renamed(&module.names);
        held.modules.push(rustmanifest_schema::ResolvedModule {
            names,
            file: origin.clone(),
            crate_root: root,
            parent: (!root).then(|| "fixture".to_owned())
        });
    }
}
