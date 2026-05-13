# rustmanifest-lsp

Language Server Protocol server for `rustmanifest`. Provides diagnostics, code actions, and hover for editors without MCP support (Zed, Helix, Neovim, VS Code via the LSP client).

Reuses `rustmanifest-engine` directly — no duplication of analysis logic between the MCP and LSP servers.

Phase 0 ships the crate skeleton only; the LSP server lands in Phase 5.
