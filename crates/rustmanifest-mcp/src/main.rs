// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Stdio entry point for the `rustmanifest` MCP server.
//!
//! The workspace root is the process current directory at startup; every
//! path received over MCP is confined to it. Tracing goes to stderr so
//! stdout stays reserved for the MCP protocol stream.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the rmcp SDK pins duplicate transitive dependencies (syn 2/3, hashbrown, \
              wit-bindgen) that cannot be deduplicated from this crate"
)]

use masterror::ResultExt;
use rmcp::{ServiceExt, transport::stdio};
use rustmanifest_mcp::server::ReviewServer;
use tracing::info;

#[tokio::main]
async fn main() -> masterror::AppResult<()> {
    init_tracing();
    let server = ReviewServer::from_current_dir()
        .context("resolving workspace root from current directory")?;
    info!("starting rustmanifest MCP server on stdio");
    let running = server
        .serve(stdio())
        .await
        .context("starting stdio MCP transport")?;
    running
        .waiting()
        .await
        .context("waiting for MCP service shutdown")?;
    Ok(())
}

/// Installs a stderr tracing subscriber honoring `RUST_LOG`.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
