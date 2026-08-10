-- SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
-- SPDX-License-Identifier: MIT

-- The rust-manifest server configuration, in the shape Neovim 0.11+
-- reads from an `lsp/` runtime directory. The server runs beside
-- rust-analyzer: both attach to the same buffer, and hover, code
-- actions, and inlay hints from each are shown by the editor.
return {
  cmd = { 'rust-manifest' },
  filetypes = { 'rust' },
  root_markers = { 'Cargo.toml', '.git' },
}
