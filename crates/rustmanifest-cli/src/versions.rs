// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `versions` subcommand: checks version synchronisation.

use std::{io::Write, process::ExitCode};

use anyhow::Context;
use clap::Parser;

use crate::EXIT_CLEAN;

/// Arguments for the `versions` subcommand.
#[derive(Debug, Parser)]
pub struct VersionsArgs {
    /// Path to the workspace root (defaults to the current directory).
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf
}

/// Runs the `versions` subcommand: prints a version-synchronisation report.
pub(crate) fn run_versions(args: &VersionsArgs) -> anyhow::Result<ExitCode> {
    let root = &args.path;
    let manifest_path = root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;

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
    let ra_aligned = ra_versions.len() <= 1;

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
    Ok(ExitCode::from(EXIT_CLEAN))
}
