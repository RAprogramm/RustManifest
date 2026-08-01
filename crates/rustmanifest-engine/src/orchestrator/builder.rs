// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Builder assembling [`Orchestrator`] instances: analyzer registration,
//! memory budget, cancellation token, and the optional findings cache
//! with its pre-computed rule-set fingerprint.

use std::sync::Arc;

use super::{Cancellation, DEFAULT_MAX_FILE_BYTES, Orchestrator};
use crate::{
    analyzer::Analyzer,
    cache::{self, FindingsCache}
};

#[derive(Debug, Clone)]
pub(super) struct CacheHandle {
    pub(super) store:       FindingsCache,
    pub(super) fingerprint: Arc<[u8]>
}

/// Builder for [`Orchestrator`] instances.
#[derive(Default)]
pub struct OrchestratorBuilder {
    analyzers:      Vec<Box<dyn Analyzer>>,
    max_file_bytes: Option<u64>,
    cancellation:   Option<Cancellation>,
    cache:          Option<FindingsCache>
}

impl std::fmt::Debug for OrchestratorBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OrchestratorBuilder")
            .field("analyzers", &self.analyzers.len())
            .field("max_file_bytes", &self.max_file_bytes)
            .field("cancellation", &self.cancellation)
            .field("cache", &self.cache)
            .finish()
    }
}

impl OrchestratorBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one analyzer with the orchestrator.
    #[must_use]
    pub fn analyzer(mut self, analyzer: Box<dyn Analyzer>) -> Self {
        self.analyzers.push(analyzer);
        self
    }

    /// Registers a batch of analyzers, consuming the iterator.
    #[must_use]
    pub fn analyzers<I>(mut self, analyzers: I) -> Self
    where
        I: IntoIterator<Item = Box<dyn Analyzer>>
    {
        self.analyzers.extend(analyzers);
        self
    }

    /// Sets the per-file memory budget. Files larger than this are skipped.
    #[must_use]
    pub const fn max_file_bytes(mut self, bytes: u64) -> Self {
        self.max_file_bytes = Some(bytes);
        self
    }

    /// Attaches a cancellation token.
    #[must_use]
    pub fn cancellation(mut self, token: Cancellation) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Attaches an optional persistent findings cache. `None` (the default)
    /// disables caching.
    ///
    /// On a cache hit the orchestrator reuses the stored per-file findings —
    /// including the `RM-META-001` unused-pragma post-pass results — and
    /// skips analysis for that file entirely. See [`crate::cache`] for the
    /// key composition and failure model; note that the workspace-level
    /// semantic tier runs outside the orchestrator and is never cached.
    #[must_use]
    pub fn cache(mut self, cache: Option<FindingsCache>) -> Self {
        self.cache = cache;
        self
    }

    /// Finalises the builder into an [`Orchestrator`].
    #[must_use]
    pub fn build(self) -> Orchestrator {
        let cache = self.cache.map(|store| {
            let rules: Vec<&rustmanifest_schema::Rule> = self
                .analyzers
                .iter()
                .map(|analyzer| analyzer.rule())
                .collect();
            CacheHandle {
                store,
                fingerprint: Arc::from(cache::rules_fingerprint(&rules))
            }
        });
        Orchestrator {
            analyzers: Arc::from(self.analyzers.into_boxed_slice()),
            max_file_bytes: self.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES),
            cancellation: self.cancellation.unwrap_or_default(),
            cache
        }
    }
}
