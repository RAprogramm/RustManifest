// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Output format selection and rendering of the collected findings.

use std::io::{IsTerminal, Write};

use clap::ValueEnum;
use masterror::ResultExt as _;
use rustmanifest_report::{JsonRenderer, Renderer, SarifRenderer, TtyRenderer};
use rustmanifest_schema::Finding;

/// Output format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Picks `tty` on a terminal, `json` otherwise.
    Auto,
    /// Canonical pretty-printed JSON array.
    Json,
    /// SARIF 2.1.0 document.
    Sarif,
    /// Human-readable terminal output.
    Tty
}

pub(super) fn render(
    findings: &[Finding],
    format: OutputFormat,
    no_color: bool
) -> masterror::AppResult<()> {
    let resolved = resolve_format(format);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match resolved {
        OutputFormat::Json | OutputFormat::Auto => {
            let mut renderer = JsonRenderer::new(&mut handle);
            renderer.render(findings).context("rendering JSON output")?;
        }
        OutputFormat::Sarif => {
            let mut renderer = SarifRenderer::new(&mut handle, env!("CARGO_PKG_VERSION"));
            renderer
                .render(findings)
                .context("rendering SARIF output")?;
        }
        OutputFormat::Tty => {
            let color = !no_color && std::io::stdout().is_terminal();
            let mut renderer = TtyRenderer::new(&mut handle).with_color(color);
            renderer.render(findings).context("rendering TTY output")?;
        }
    }
    handle.flush().context("flushing stdout")?;
    Ok(())
}

fn resolve_format(format: OutputFormat) -> OutputFormat {
    if format == OutputFormat::Auto {
        if std::io::stdout().is_terminal() {
            OutputFormat::Tty
        } else {
            OutputFormat::Json
        }
    } else {
        format
    }
}
