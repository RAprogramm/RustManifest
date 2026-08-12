// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The extraction genre's candidates: every top-level function the
//! planner confirms a whole module extraction for, up to the stated
//! cap — the rewritten parent and the planted module file together.

use std::path::Path;

use rustmanifest_engine::{Source, walker};

use super::candidate::{Candidate, Planted};
use crate::error::EvalError;

/// Lists up to `limit` confirmed extractions over the tree at `root`.
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
    let mut found = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        if found.len() >= limit {
            break;
        }
        let Ok(parsed) = syn::parse_file(source.text()) else {
            continue;
        };
        for item in &parsed.items {
            if found.len() >= limit {
                break;
            }
            let syn::Item::Fn(function) = item else {
                continue;
            };
            let start = syn::spanned::Spanned::span(function).byte_range().start;
            let offset = u32::try_from(start).unwrap_or(u32::MAX);
            let Some(plan) =
                rustmanifest_engine::semantic::extract_function(&sources, index, offset)
            else {
                continue;
            };
            found.push(planned(root, source, &function.sig.ident.to_string(), plan));
        }
    }
    Ok(found)
}

/// One extraction candidate: the parent rewritten by the plan's edits
/// and the module file the plan plants.
fn planned(
    root: &Path,
    source: &Source,
    name: &str,
    plan: rustmanifest_engine::semantic::Extraction
) -> Candidate {
    let mut installed = source.text().to_owned();
    let mut edits = plan.edits;
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in edits {
        installed.replace_range(range.start as usize..range.end as usize, &replacement);
    }
    let shown = source
        .path()
        .strip_prefix(root)
        .unwrap_or_else(|_| source.path());
    Candidate {
        subject: format!("fn {name} in {}", shown.display()),
        file: source.path().to_path_buf(),
        installed,
        planted: Some(Planted {
            file: plan.new_file,
            text: plan.new_text
        })
    }
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

    /// A tree offering one extractable function lists the rewritten
    /// parent together with the planted module file.
    #[test]
    fn a_tree_lists_its_confirmed_extractions_with_the_planted_file() {
        let bed = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(bed.path().join("src")).unwrap();
        std::fs::write(
            bed.path().join("src/lib.rs"),
            "pub fn keep() -> u8 { 1 }\n\npub fn render() -> u8 { keep() }\n"
        )
        .unwrap();
        let all = listed(bed.path(), 10).unwrap();
        assert!(!all.is_empty());
        let held = &all[0];
        assert!(held.subject.starts_with("fn "), "{}", held.subject);
        let planted = held.planted.as_ref().unwrap();
        assert!(planted.text.contains("pub fn"), "{}", planted.text);
        assert!(held.installed.contains("mod "), "{}", held.installed);
    }
}
