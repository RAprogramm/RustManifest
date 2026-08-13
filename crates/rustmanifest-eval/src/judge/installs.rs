// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The install genre's candidates: every type the montage confirms a
//! canon installation for, up to the stated cap.

use std::path::Path;

use rustmanifest_engine::{Source, walker};

use super::candidate::Candidate;
use crate::error::EvalError;

/// Lists up to `limit` confirmed installations over the tree at `root`.
///
/// # Errors
///
/// Returns an [`EvalError`] when file discovery fails.
pub(super) fn listed(root: &Path, limit: usize) -> Result<Vec<Candidate>, EvalError> {
    let files = walker::discover(std::slice::from_ref(&root.to_path_buf()))?;
    let sources: Vec<Source> = files
        .iter()
        .filter_map(|file| {
            std::fs::read_to_string(file)
                .ok()
                .map(|text| Source::new(file.clone(), text))
        })
        .collect();
    let types = rustmanifest_engine::TypeGraph::from_sources(&sources);
    let mut found = Vec::new();
    for node in types.types() {
        if found.len() >= limit {
            break;
        }
        for trait_name in ["Display", "Default"] {
            if found.len() >= limit {
                break;
            }
            if types.ability_given(&node.name, trait_name) {
                continue;
            }
            let Some(gain) = rustmanifest_engine::semantic::install_impl(
                &sources,
                None,
                node.source,
                &node.name,
                trait_name
            ) else {
                continue;
            };
            found.push(Candidate {
                subject:   format!("{} {trait_name}", node.name),
                file:      sources[node.source].path().to_path_buf(),
                installed: gain.installed,
                planted:   None
            });
        }
    }
    Ok(found)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::listed;

    /// A tree offering one provable installation lists it with the
    /// landed text, and the cap caps.
    #[test]
    fn a_tree_lists_its_confirmed_installations_under_the_cap() {
        let bed = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(bed.path().join("src")).unwrap();
        std::fs::write(
            bed.path().join("src/lib.rs"),
            "pub struct User {\n    pub name: String,\n}\n"
        )
        .unwrap();
        let all = listed(bed.path(), 10).unwrap();
        assert_eq!(all.len(), 2, "Display and Default");
        assert!(all[0].installed.contains("impl std::fmt::Display for User"));
        let capped = listed(bed.path(), 1).unwrap();
        assert_eq!(capped.len(), 1);
    }
}
