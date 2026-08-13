// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What cargo says about the packages around the analyzed paths.
//!
//! Every question here — which packages exist, what each is called, which
//! crates each depends on, what its library target is named — is one cargo
//! already answers, and answers correctly for the cases a text scan cannot
//! reach: an inherited workspace dependency, a renamed dependency, a
//! target-specific one, a path dependency outside the walked tree. This
//! module asks cargo and reads the answer.
//!
//! The previous reading walked TOML section headers line by line. It was
//! wrong on all four of those and had no way to become right.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf}
};

use cargo_metadata::MetadataCommand;

/// Every workspace root reachable from `paths`, deepest first, so a
/// nested workspace is asked about before the tree that contains it.
#[must_use]
pub fn manifest_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        walk(path, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

/// Recursive step of [`manifest_files`].
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let manifest = dir.join("Cargo.toml");
    if manifest.is_file() {
        files.push(manifest);
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !skipped_dir(&path) {
            walk(&path, files);
        }
    }
}

/// Returns whether a directory is hidden or a build-output directory.
fn skipped_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| name.starts_with('.') || name == "target")
}

/// Every crate name cargo reports around `manifest`, in the spelling a
/// `use` path writes: the packages of the workspace, their library
/// targets, and every dependency each one declares.
///
/// Nothing where cargo refuses to answer — a manifest it cannot resolve
/// is one this tool has nothing true to say about.
#[must_use]
pub fn known_crate_names(manifest: &Path) -> BTreeSet<String> {
    let Ok(metadata) = MetadataCommand::new()
        .manifest_path(manifest)
        .no_deps()
        .exec()
    else {
        return BTreeSet::new();
    };
    let mut names = BTreeSet::new();
    for package in &metadata.packages {
        names.insert(normalized(&package.name));
        for target in &package.targets {
            if target.is_lib() {
                names.insert(normalized(&target.name));
            }
        }
        for dependency in &package.dependencies {
            names.insert(normalized(
                dependency.rename.as_ref().unwrap_or(&dependency.name)
            ));
        }
    }
    names
}

/// The name cargo gives the package whose manifest is `manifest`.
///
/// Matched by manifest path rather than by the workspace root, because a
/// member of a workspace is a package while the root above it may declare
/// none — and both answer the same metadata query.
#[must_use]
pub fn package_name(manifest: &Path) -> Option<String> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .no_deps()
        .exec()
        .ok()?;
    let asked = manifest.canonicalize().ok()?;
    metadata
        .packages
        .iter()
        .find(|package| {
            package
                .manifest_path
                .canonicalize()
                .is_ok_and(|held| held == asked)
        })
        .map(|package| package.name.to_string())
}

/// A crate name as a `use` path spells it.
fn normalized(name: &str) -> String {
    name.replace('-', "_")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct TestWorkspace {
        root: TempDir,
    }

    impl TestWorkspace {
        fn new() -> Self {
            Self {
                root: TempDir::new().unwrap(),
            }
        }

        fn write_manifest(&self, rel_path: &str, content: &str) {
            let path = self.root.path().join(rel_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }

        fn manifest_path(&self, rel_path: &str) -> PathBuf {
            self.root.path().join(rel_path)
        }

        fn root_path(&self) -> PathBuf {
            self.root.path().to_path_buf()
        }
    }

    #[test]
    fn manifest_files_finds_root_manifest() {
        let ws = TestWorkspace::new();
        ws.write_manifest("Cargo.toml", "[workspace]\nresolver = \"2\"\n");
        let files = manifest_files(&[ws.root_path()]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], ws.manifest_path("Cargo.toml"));
    }

    #[test]
    fn manifest_files_finds_member_manifests() {
        let ws = TestWorkspace::new();
        ws.write_manifest(
            "Cargo.toml",
            "[workspace]\nresolver = \"2\"\nmembers = [\"a\", \"b\"]\n",
        );
        ws.write_manifest(
            "a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1\"\nedition = \"2024\"\n",
        );
        ws.write_manifest(
            "b/Cargo.toml",
            "[package]\nname = \"b\"\nversion = \"0.1\"\nedition = \"2024\"\n",
        );
        let files = manifest_files(&[ws.root_path()]);
        assert_eq!(files.len(), 3);
        assert!(files.contains(&ws.manifest_path("Cargo.toml")));
        assert!(files.contains(&ws.manifest_path("a/Cargo.toml")));
        assert!(files.contains(&ws.manifest_path("b/Cargo.toml")));
    }

    #[test]
    fn manifest_files_skips_target_directory() {
        let ws = TestWorkspace::new();
        ws.write_manifest("Cargo.toml", "[workspace]\nresolver = \"2\"\n");
        ws.write_manifest(
            "target/debug/build/gen/Cargo.toml",
            "[package]\nname = \"gen\"\nversion = \"0.1\"\nedition = \"2024\"\n",
        );
        let files = manifest_files(&[ws.root_path()]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], ws.manifest_path("Cargo.toml"));
    }

    #[test]
    fn manifest_files_skips_hidden_directories() {
        let ws = TestWorkspace::new();
        ws.write_manifest("Cargo.toml", "[workspace]\nresolver = \"2\"\n");
        ws.write_manifest(
            ".cargo/Cargo.toml",
            "[package]\nname = \"hidden\"\nversion = \"0.1\"\nedition = \"2024\"\n",
        );
        let files = manifest_files(&[ws.root_path()]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], ws.manifest_path("Cargo.toml"));
    }

    #[test]
    fn known_crate_names_empty_on_unresolvable_manifest() {
        let path = PathBuf::from("/nonexistent/Cargo.toml");
        let names = known_crate_names(&path);
        assert!(names.is_empty());
    }

    #[test]
    fn package_name_returns_none_for_workspace_root() {
        let ws = TestWorkspace::new();
        ws.write_manifest("Cargo.toml", "[workspace]\nresolver = \"2\"\nmembers = []\n");
        assert!(package_name(&ws.manifest_path("Cargo.toml")).is_none());
    }

    #[test]
    fn package_name_returns_none_for_unresolvable_manifest() {
        let path = PathBuf::from("/nonexistent/Cargo.toml");
        assert!(package_name(&path).is_none());
    }
}
