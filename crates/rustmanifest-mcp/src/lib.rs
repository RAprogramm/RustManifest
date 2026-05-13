// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! MCP server for `rustmanifest`.
//!
//! Phase 0 ships the crate skeleton only. The server implementation, built on
//! the `rmcp` SDK, lands in Phase 2 (stdio transport) and Phase 3 (Streamable
//! HTTP with OAuth 2.1).

/// MCP protocol revision targeted by this server.
pub const PROTOCOL_REVISION: &str = "2025-06-18";
