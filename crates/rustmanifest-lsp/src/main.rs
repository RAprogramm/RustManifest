// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The `rustmanifest-lsp` binary: the server over standard input and
//! output, which is the transport every editor speaks first.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    let (connection, threads) = lsp_server::Connection::stdio();
    let served = rustmanifest_lsp::serve(&connection);
    let joined = threads.join();
    match (served, joined) {
        (Ok(()), Ok(())) => ExitCode::SUCCESS,
        (Err(error), _) => {
            tracing::error!(error = %format!("{error:#}"), "rustmanifest-lsp failed");
            ExitCode::FAILURE
        }
        (_, Err(error)) => {
            tracing::error!(error = %format!("{error:#}"), "rustmanifest-lsp transport failed");
            ExitCode::FAILURE
        }
    }
}
