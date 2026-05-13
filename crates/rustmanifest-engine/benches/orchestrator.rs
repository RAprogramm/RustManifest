// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Criterion benchmarks for orchestrator throughput.
//!
//! Generates a synthetic in-memory file set, writes it to a tempdir, and
//! measures wall-clock time for a full orchestrator pass with the canonical
//! `RM-SEC-001` rule. Benchmarks are compile-gated by CI (no gate on
//! results yet — a perf budget lands with Phase 1E's eval corpus).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_docs_in_private_items,
    reason = "benchmark code is allowed to fail loudly on broken invariants"
)]

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf}
};

use criterion::{Criterion, criterion_group, criterion_main};
use rustmanifest_engine::{OrchestratorBuilder, PatternAnalyzer};
use rustmanifest_schema::{Rule, RuleDefinition, Severity};
use tempfile::TempDir;

fn rule() -> Rule {
    Rule {
        id:            "RM-SEC-001".to_owned(),
        severity:      Severity::Error,
        title:         "Hardcoded credentials".to_owned(),
        rationale_uri: "rustmanifest://methodology/security#creds".to_owned(),
        definition:    RuleDefinition::Pattern {
            regex:         r#"(?i)password\s*=\s*"[^"]+""#.to_owned(),
            exclude_globs: vec![]
        }
    }
}

fn populate(dir: &Path, file_count: usize) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(file_count);
    for idx in 0..file_count {
        let path = dir.join(format!("file_{idx}.rs"));
        let mut handle = fs::File::create(&path).unwrap();
        let payload = if idx.is_multiple_of(2) {
            "fn main() { let password = \"hunter2\"; }\n".to_owned()
        } else {
            "fn main() { let validated = compute(); }\n".to_owned()
        };
        handle.write_all(payload.as_bytes()).unwrap();
        paths.push(path);
    }
    paths
}

fn bench_orchestrator(criterion: &mut Criterion) {
    for &count in &[16usize, 64, 256] {
        let dir = TempDir::new().unwrap();
        let files = populate(dir.path(), count);
        let analyzer = PatternAnalyzer::new(rule()).unwrap();
        let orchestrator = OrchestratorBuilder::new()
            .analyzer(Box::new(analyzer))
            .build();

        criterion.bench_function(&format!("orchestrator_{count}_files"), |bencher| {
            bencher.iter(|| {
                orchestrator.run(&files).unwrap();
            });
        });
    }
}

criterion_group!(benches, bench_orchestrator);
criterion_main!(benches);
