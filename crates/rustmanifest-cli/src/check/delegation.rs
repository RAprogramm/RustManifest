// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Delegation to `cargo clippy` and translation of its diagnostics into
//! advisory findings.

use std::path::{Path, PathBuf};

use rustmanifest_schema::{Finding, Location, Range, Severity};
use tracing::warn;

/// Prefix of the clippy lint index page used to build rationale URIs for
/// delegated findings.
const CLIPPY_INDEX_URL: &str = "https://rust-lang.github.io/rust-clippy/master/index.html#";

/// Listing of the compiler's own lints, used as the rationale of a
/// delegated rustc diagnostic.
const RUSTC_LINT_INDEX_URL: &str = "https://doc.rust-lang.org/rustc/lints/listing/index.html";

/// Runs `cargo clippy --message-format=json --quiet` in the first analyzed
/// path and converts its diagnostics into advisory findings, sorted by
/// `(file, range start, rule id)` for deterministic output.
///
/// The manifest probe is intentionally shallow: only `<path>/Cargo.toml` is
/// checked and parent workspace manifests are not searched. Every failure
/// mode — missing `cargo`, a non-zero exit without parseable diagnostics,
/// malformed JSON lines — degrades to a [`warn`] log entry; delegation
/// never fails the surrounding check run. The child process inherits the
/// caller's environment untouched except for `CARGO_TERM_COLOR=never`.
pub(super) fn delegated_clippy_findings(paths: &[PathBuf]) -> Vec<Finding> {
    let Some(root) = paths.first() else {
        warn!("clippy delegation skipped: no analysis paths were given");
        return Vec::new();
    };
    if !root.join("Cargo.toml").is_file() {
        warn!(
            path = %root.display(),
            "clippy delegation skipped: no Cargo.toml in the analyzed path"
        );
        return Vec::new();
    }
    let output = match std::process::Command::new("cargo")
        .args(["clippy", "--message-format=json", "--quiet"])
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            warn!(%error, "clippy delegation skipped: failed to invoke cargo");
            return Vec::new();
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut findings: Vec<Finding> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| parse_clippy_line(line, root))
        .collect();
    if !output.status.success() && findings.is_empty() {
        warn!(
            status = %output.status,
            "cargo clippy exited unsuccessfully without parseable clippy diagnostics"
        );
    }
    findings.sort_by(|left, right| {
        (
            &left.location.file,
            left.location.range.start,
            &left.rule_id,
        )
            .cmp(&(
                &right.location.file,
                right.location.range.start,
                &right.rule_id,
            ))
    });
    findings
}

/// Parses one line of `cargo clippy --message-format=json` output into a
/// [`Finding`].
///
/// Returns `None` for lines that are not diagnostics: other cargo message
/// kinds, messages carrying no lint code, and summary messages without a
/// primary span. Lines that are not valid JSON are logged as a [`warn`]
/// entry and skipped.
///
/// The compiler's own lints are taken as well as clippy's. Dropping them
/// was what made this project write its own unreferenced-function check,
/// its own unused-import census and its own reachability walk — every one
/// of them a worse answer to a question already in the same output
/// stream.
fn parse_clippy_line(line: &str, root: &Path) -> Option<Finding> {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            warn!(%error, "skipping malformed cargo clippy JSON line");
            return None;
        }
    };
    if value.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-message") {
        return None;
    }
    let message = value.get("message")?;
    let code = message.get("code")?.get("code")?.as_str()?;
    let rationale = code.strip_prefix("clippy::").map_or_else(
        || RUSTC_LINT_INDEX_URL.to_owned(),
        |anchor| format!("{CLIPPY_INDEX_URL}{anchor}"),
    );
    let severity = match message.get("level").and_then(serde_json::Value::as_str) {
        Some("error") => Severity::Error,
        Some("warning") => Severity::Warning,
        _ => Severity::Info,
    };
    let spans = message.get("spans")?.as_array()?;
    let primary = spans
        .iter()
        .find(|span| span.get("is_primary").and_then(serde_json::Value::as_bool) == Some(true))?;
    let file_name = primary
        .get("file_name")
        .and_then(serde_json::Value::as_str)?;
    let file = Path::new(file_name)
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(file_name))
        .to_path_buf();
    let start = primary
        .get("byte_start")
        .and_then(serde_json::Value::as_u64)?;
    let end = primary
        .get("byte_end")
        .and_then(serde_json::Value::as_u64)?;
    let text = message.get("message").and_then(serde_json::Value::as_str)?;
    Some(Finding {
        rule_id: code.to_owned(),
        severity,
        location: Location {
            file,
            range: Range {
                start: u32::try_from(start).ok()?,
                end: u32::try_from(end).ok()?,
            },
        },
        message: text.to_owned(),
        rationale_uri: rationale,
        trace: Vec::new(),
        fixes: Vec::new(),
    })
}
