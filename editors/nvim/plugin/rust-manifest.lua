-- SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
-- SPDX-License-Identifier: MIT

-- Enables the rust-manifest language server where the runtime can
-- carry it: Neovim 0.11+ with the binary on the PATH. Silence
-- otherwise — a plugin that errors at startup costs more than the
-- feature it gates.
if vim.fn.has('nvim-0.11') == 1 and vim.fn.executable('rust-manifest') == 1 then
  vim.lsp.enable('rust_manifest')
end
