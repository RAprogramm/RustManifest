// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cooperative cancellation for orchestrator runs.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
};

/// Cooperative cancellation handle threaded through the orchestrator.
#[derive(Debug, Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>
}

impl Cancellation {
    /// Creates a fresh, non-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals cancellation. Subsequent orchestrator iterations will return
    /// a cancellation error.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    /// Returns true if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}
