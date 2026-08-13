// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a compiler settles about a test fixture, asked of the compiler.
//!
//! Every reading of the engine answers from a resolution and from
//! nothing else, so a test over a written snippet has to state the
//! compiler's side of it. Writing that side down by hand is the very
//! thing the engine stopped doing: a table of names knows what somebody
//! thought to add and is silently wrong for the rest.
//!
//! So the fixture is built into a package and handed to the resolver.
//! The answer is the compiler's own, and a test that passes against it
//! passes for the reason it claims.

use std::path::{Path, PathBuf};

use rustmanifest_schema::Resolution;

/// What the compiler settles about `files`, keyed by the paths they are
/// written under.
///
/// The files are laid out as one package — the resolver reads a cargo
/// workspace and nothing else — and the answer is rebased onto those
/// same written paths, so a reader holding the snippets by their own
/// names finds every row.
///
/// # Panics
///
/// Panics when the temporary package cannot be written or the resolver
/// refuses it: a fixture whose compiler answer cannot be had is a test
/// that would otherwise pass for the wrong reason.
#[must_use]
pub fn settled(files: &[(&str, &str)]) -> Resolution {
    let key = keyed(files);
    if let Some(held) = cached(&key) {
        return held;
    }
    let held = asked(files);
    keep(&key, &held);
    held
}

/// The answer read back from the cache, where a run already asked the
/// compiler about the very same snippets.
///
/// Asking costs seconds — the resolver loads the language's own
/// libraries before it can answer anything — and a suite asks about the
/// same few snippets over and over. The cache is keyed by the snippets
/// themselves, so an answer is reused exactly when the question is the
/// same one.
fn cached(key: &str) -> Option<Resolution> {
    let text = std::fs::read_to_string(kept_at(key)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes one answer back for the next run to read.
fn keep(key: &str, held: &Resolution) {
    let at = kept_at(key);
    if let Some(parent) = at.parent() {
        drop(std::fs::create_dir_all(parent));
    }
    if let Ok(text) = serde_json::to_string(held) {
        drop(std::fs::write(at, text));
    }
}

/// Where one answer is kept: beside the build output, so a clean build
/// asks again and nothing stale outlives the resolver it came from.
fn kept_at(key: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fixture-answers")
        .join(format!("{key}.json"))
}

/// The key one set of snippets is asked and answered under: the
/// resolver's own version and the snippets themselves.
fn keyed(files: &[(&str, &str)]) -> String {
    use std::hash::{Hash as _, Hasher as _};

    let mut held = std::collections::hash_map::DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut held);
    for (path, text) in files {
        path.hash(&mut held);
        text.hash(&mut held);
    }
    format!("{:016x}", held.finish())
}

/// What the compiler says, asked outright.
fn asked(files: &[(&str, &str)]) -> Resolution {
    let bed = tempfile::tempdir().expect("a temporary package");
    write_package(bed.path(), files);
    let mut held = rustmanifest_resolve::resolve(bed.path()).expect("the resolver answers");
    let written: Vec<(PathBuf, PathBuf)> = files
        .iter()
        .map(|(path, _)| {
            (
                PathBuf::from("src").join(module_of(path)),
                PathBuf::from(path)
            )
        })
        .collect();
    rename(&mut held, &written);
    held.settle();
    held
}


/// Puts every row on the path its snippet was written under, and drops
/// the rows of the crate root the package needed.
fn rename(held: &mut Resolution, written: &[(PathBuf, PathBuf)]) {
    let named = |file: &PathBuf| -> Option<PathBuf> {
        written
            .iter()
            .find(|(laid, _)| laid == file)
            .map(|(_, path)| path.clone())
    };
    held.calls.retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.fields.retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.types.retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.paths.retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.functions
        .retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.signatures
        .retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.declared
        .retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.grants.retain_mut(|row| rename_at(&mut row.at.file, &named));
    held.modules.retain_mut(|row| rename_at(&mut row.file, &named));
}

/// Renames one row's file, answering whether the row belongs to a
/// snippet at all.
fn rename_at(file: &mut PathBuf, named: &impl Fn(&PathBuf) -> Option<PathBuf>) -> bool {
    match named(file) {
        Some(path) => {
            *file = path;
            true
        }
        None => false
    }
}

/// Writes `files` as one package the resolver can read.
fn write_package(root: &Path, files: &[(&str, &str)]) {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\npath \
         = \"src/lib.rs\"\n"
    )
    .expect("the manifest is written");
    std::fs::create_dir_all(root.join("src")).expect("the source directory");
    let mut declared = String::new();
    for (path, text) in files {
        let at = root.join("src").join(module_of(path));
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("the module directory");
        std::fs::write(&at, text).expect("the module is written");
        declared.push_str(&format!(
            "#[path = \"{}\"]\npub mod {};\n",
            module_of(path),
            module_name(path)
        ));
    }
    std::fs::write(root.join("src/lib.rs"), declared).expect("the crate root is written");
}

/// The file name a written path is laid out under inside the package.
fn module_of(path: &str) -> String {
    format!("{}.rs", module_name(path))
}

/// The module name a written path is declared as.
fn module_name(path: &str) -> String {
    let stem = path
        .trim_end_matches(".rs")
        .replace(['/', '.', '-'], "_");
    format!("held_{stem}")
}


