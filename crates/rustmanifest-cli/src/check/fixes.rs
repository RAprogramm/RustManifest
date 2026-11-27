// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Application of machine-applicable fixes to the analyzed sources.

use std::{collections::HashMap, path::Path};

use masterror::ResultExt as _;
use rustmanifest_schema::Finding;
use tracing::warn;

/// Applies every machine-applicable fix, grouped per file, edits sorted
/// descending by start so earlier offsets stay valid. Returns how many
/// fixes were applied; a file that changed on disk is skipped honestly.
pub(super) fn apply_machine_fixes(findings: &[Finding]) -> masterror::AppResult<usize> {
    use rustmanifest_schema::Applicability;
    let mut per_file: HashMap<&Path, Vec<&rustmanifest_schema::Fix>> = HashMap::new();
    for finding in findings {
        for fix in &finding.fixes {
            if fix.applicability == Applicability::MachineApplicable {
                let target = fix
                    .file
                    .as_deref()
                    .unwrap_or(finding.location.file.as_path());
                per_file.entry(target).or_default().push(fix);
            }
        }
    }
    let mut applied = 0usize;
    let mut renames: Vec<(&Path, &Path)> = Vec::new();
    let mut files: Vec<&Path> = per_file.keys().copied().collect();
    files.sort_unstable();
    for path in files {
        let mut edits = per_file.remove(path).unwrap_or_default();
        edits.retain(|edit| {
            if let Some(target) = edit.rename_to.as_deref() {
                renames.push((path, target));
                return false;
            }
            true
        });
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));
        edits.dedup_by(|a, b| a.range == b.range && a.replacement == b.replacement);
        let mut text = std::fs::read_to_string(path)
            .context(format!("reading {} for fixing", path.display()))?;
        let mut touched = false;
        for edit in edits {
            let (start, end) = (edit.range.start as usize, edit.range.end as usize);
            if end > text.len() || start > end {
                warn!(path = %path.display(), "fix range out of bounds; skipped");
                continue;
            }
            text.replace_range(start..end, &edit.replacement);
            touched = true;
            applied += 1;
        }
        if touched {
            std::fs::write(path, text).context(format!("writing fixed {}", path.display()))?;
        }
    }
    renames.sort_unstable();
    renames.dedup();
    for (from, to) in renames {
        if to.exists() {
            warn!(from = %from.display(), to = %to.display(), "rename target exists; skipped");
            continue;
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).context(format!("creating {}", parent.display()))?;
        }
        std::fs::rename(from, to).context(format!(
            "renaming {} to {}",
            from.display(),
            to.display()
        ))?;
        applied += 1;
    }
    Ok(applied)
}
