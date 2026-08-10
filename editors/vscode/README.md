# rust-manifest for VS Code

Runs the `rust-manifest` language server for Rust files: fact cards on
hover, ownership stories at written calls, ghost-text ownership marks,
diagnostics with quickfixes, and the extraction and implement actions.
It runs beside rust-analyzer — the editor shows both hovers and merges
both action lists.

## Requirements

The server binary: `cargo install rustmanifest-lsp` (installs
`rust-manifest`). The extension does not bundle it; set
`rust-manifest.path` if the binary is outside `PATH`.

## Build and package

```sh
npm install
npm run compile
npm run package
```

The produced `.vsix` installs via `code --install-extension` and is
publishable to the Visual Studio Marketplace (`vsce publish`) and to
Open VSX (`npx ovsx publish`).

## Settings

- `rust-manifest.enable` — start the server (default `true`).
- `rust-manifest.path` — binary path (default `rust-manifest`).
