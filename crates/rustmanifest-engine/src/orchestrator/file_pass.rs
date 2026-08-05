// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Single-file analysis pass: the stage that turns one path into findings.
//!
//! The pass enforces the per-file memory budget, loads the text, consults
//! the optional findings cache, fans the analyzer set out over the loaded
//! [`Source`], appends the unused-pragma post-pass and stores the result
//! back into the cache. Every IO failure is demoted to a warning and the
//! file is skipped, so a broken path never fails a whole run.

use std::{fs, path::Path};

use rustmanifest_schema::Finding;
use tracing::{debug, trace, warn};

use super::Orchestrator;
use crate::{cache, source::Source};

/// Default per-file memory budget: 10 MiB.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

impl Orchestrator {
    pub(super) fn analyze_one_file(&self, path: &Path) -> Vec<Finding> {
        if self.cancellation.is_cancelled() {
            return Vec::new();
        }

        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(err) => {
                warn!(path = %path.display(), error = %err, "metadata read failed");
                return Vec::new();
            }
        };
        if metadata.len() > self.max_file_bytes {
            warn!(
                path = %path.display(),
                bytes = metadata.len(),
                limit = self.max_file_bytes,
                "file exceeds memory budget; skipped"
            );
            return Vec::new();
        }

        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                warn!(path = %path.display(), error = %err, "read failed; skipped");
                return Vec::new();
            }
        };

        let cache_key = self
            .cache
            .as_ref()
            .map(|handle| cache::key(text.as_bytes(), &handle.fingerprint));
        if let (Some(handle), Some(key)) = (&self.cache, &cache_key)
            && let Some(findings) = handle.store.lookup(key, path, text.len())
        {
            debug!(path = %path.display(), findings = findings.len(), "served from cache");
            return findings;
        }

        let source = Source::new(path.to_path_buf(), text);
        let mut findings = Vec::new();
        for analyzer in self.analyzers.as_ref() {
            if analyzer.is_excluded(path) {
                trace!(
                    rule = %analyzer.rule().id,
                    path = %path.display(),
                    "excluded by rule glob"
                );
                continue;
            }
            findings.extend(analyzer.analyze(&source));
        }
        findings.extend(self.unused_pragma_findings(&source));
        if let (Some(handle), Some(key)) = (&self.cache, &cache_key) {
            handle.store.store(key, source.text().len(), &findings);
        }
        debug!(path = %path.display(), findings = findings.len(), "file analyzed");
        findings
    }
}
