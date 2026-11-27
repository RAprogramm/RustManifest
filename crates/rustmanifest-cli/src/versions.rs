// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `versions` subcommand: checks version synchronisation.

use std::{io::Write, process::ExitCode};

use clap::Parser;
use masterror::ResultExt as _;

use crate::{EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for the `versions` subcommand.
#[derive(Debug, Parser)]
pub struct VersionsArgs {
    /// Path to the workspace root (defaults to the current directory).
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,

    /// Apply machine-applicable fixes (align `ra_ap` versions) when possible.
    #[arg(long)]
    pub fix: bool
}

/// Returns the distinct `ra_ap` versions found in workspace `Cargo.toml` when
/// they are misaligned, otherwise `None`.
pub(crate) fn ra_ap_misaligned(
    root: &std::path::Path
) -> masterror::AppResult<Option<Vec<String>>> {
    let manifest_path = root.join("Cargo.toml");
    let manifest = match std::fs::read_to_string(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context(format!("reading {}", manifest_path.display()));
        }
    };
    let mut ra_versions: Vec<String> = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ra_ap_")
            && let Some(value) = trimmed.split('=').nth(1)
        {
            let value = value.trim().trim_matches('"').to_owned();
            if !value.is_empty() && !ra_versions.contains(&value) {
                ra_versions.push(value);
            }
        }
    }
    if ra_versions.len() <= 1 {
        Ok(None)
    } else {
        Ok(Some(ra_versions))
    }
}

/// Runs the `versions` subcommand: prints a version-synchronisation report.
pub(crate) fn run_versions(args: &VersionsArgs) -> masterror::AppResult<ExitCode> {
    let root = &args.path;
    let manifest_path = root.join("Cargo.toml");
    let mut manifest = std::fs::read_to_string(&manifest_path)
        .context(format!("reading {}", manifest_path.display()))?;

    let msrv = manifest
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("rust-version")
                .and_then(|rest| rest.trim().strip_prefix('='))
                .map(|value| value.trim().trim_matches('"').to_owned())
        })
        .unwrap_or_else(|| "unset".to_owned());

    let toolchain_path = root.join("rust-toolchain.toml");
    let toolchain = std::fs::read_to_string(&toolchain_path).ok();
    let channel = toolchain.as_deref().and_then(|text| {
        text.lines().find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("channel")
                .and_then(|rest| rest.trim().strip_prefix('='))
                .map(|value| value.trim().trim_matches('"').to_owned())
        })
    });

    let mut ra_versions: Vec<String> = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ra_ap_")
            && let Some(value) = trimmed.split('=').nth(1)
        {
            let value = value.trim().trim_matches('"').to_owned();
            if !value.is_empty() && !ra_versions.contains(&value) {
                ra_versions.push(value);
            }
        }
    }
    let mut ra_aligned = ra_versions.len() <= 1;
    let fixed = if !ra_aligned && args.fix {
        let canonical = canonical_ra_version(&manifest).unwrap_or_else(|| ra_versions[0].clone());
        manifest = align_ra_versions(&manifest, &canonical);
        std::fs::write(&manifest_path, &manifest)
            .context(format!("writing {}", manifest_path.display()))?;
        ra_versions.clear();
        ra_versions.push(canonical);
        ra_aligned = true;
        true
    } else {
        false
    };

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "## versions").context("writing output")?;
    writeln!(handle, "- msrv: {msrv}").context("writing output")?;
    writeln!(
        handle,
        "- toolchain channel: {}",
        channel.as_deref().unwrap_or("unset")
    )
    .context("writing output")?;
    writeln!(
        handle,
        "- ra_ap alignment: {}",
        if ra_aligned { "aligned" } else { "MISALIGNED" }
    )
    .context("writing output")?;
    if fixed {
        writeln!(handle, "- applied fix: aligned ra_ap versions").context("writing output")?;
    }
    if !ra_aligned {
        writeln!(handle, "  ra_ap versions found: {}", ra_versions.join(", "))
            .context("writing output")?;
    }
    if channel.as_deref() != Some("stable") && channel.as_deref() != Some(&msrv) {
        writeln!(
            handle,
            "- note: toolchain channel ({}) differs from msrv ({msrv})",
            channel.as_deref().unwrap_or("unset")
        )
        .context("writing output")?;
    }
    handle.flush().context("flushing stdout")?;
    if ra_aligned {
        Ok(ExitCode::from(EXIT_CLEAN))
    } else {
        Ok(ExitCode::from(EXIT_FINDINGS))
    }
}

/// Returns the canonical `ra_ap` version: `ra_ap_ide` if present, otherwise the
/// first `ra_ap` dependency found.
fn canonical_ra_version(manifest: &str) -> Option<String> {
    let mut first = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("ra_ap_") {
            continue;
        }
        let value = trimmed
            .split('=')
            .nth(1)?
            .trim()
            .trim_matches('"')
            .to_owned();
        if value.is_empty() {
            continue;
        }
        if trimmed.starts_with("ra_ap_ide ") || trimmed.starts_with("ra_ap_ide=") {
            return Some(value);
        }
        if first.is_none() {
            first = Some(value);
        }
    }
    first
}

/// Rewrites every `ra_ap_*` dependency version in the manifest to `canonical`.
fn align_ra_versions(manifest: &str, canonical: &str) -> String {
    let mut output = String::with_capacity(manifest.len());
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ra_ap_") && trimmed.contains('=') {
            let replaced = replace_ra_version(line, canonical);
            output.push_str(&replaced);
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    if !manifest.ends_with('\n') {
        output.pop();
    }
    output
}

/// Replaces the version string in a single `ra_ap` dependency line.
fn replace_ra_version(line: &str, canonical: &str) -> String {
    if let Some(start) = line.find('"')
        && let Some(end) = line[start + 1..].find('"')
    {
        let end = start + 1 + end;
        let mut replaced = String::with_capacity(line.len());
        replaced.push_str(&line[..=start]);
        replaced.push_str(canonical);
        replaced.push_str(&line[end..]);
        return replaced;
    }
    line.to_owned()
}
