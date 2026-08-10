# rust-manifest for Neovim

Runs the `rust-manifest` language server beside rust-analyzer: fact
cards on hover, ownership stories at written calls, ghost-text
ownership marks, diagnostics with quickfixes, and the extraction and
implement actions.

## Requirements

- Neovim 0.11 or newer (the native `vim.lsp.enable` configuration).
- The server binary: `cargo install rustmanifest-lsp` (installs
  `rust-manifest`).

## Install

With lazy.nvim, point at this repository and narrow the runtime path to
this directory:

```lua
{
  'RAprogramm/RustManifest',
  config = function(plugin)
    vim.opt.rtp:append(plugin.dir .. '/editors/nvim')
    vim.lsp.enable('rust_manifest')
  end,
}
```

Or copy `lsp/rust_manifest.lua` into `~/.config/nvim/lsp/` and add one
line to your `init.lua`:

```lua
vim.lsp.enable('rust_manifest')
```

## Living beside rust-analyzer

Both servers attach to the same buffer. Hover shows both cards,
`vim.lsp.buf.code_action()` merges the action lists, and inlay hints
render from every attached server once `vim.lsp.inlay_hint.enable(true)`
is set. Nothing needs disabling on either side.
