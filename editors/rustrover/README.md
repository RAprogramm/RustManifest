# rust-manifest for RustRover

A plugin starting the `rust-manifest` language server beside
RustRover's built-in Rust support: fact cards on hover, ownership
stories at written calls, ghost-text ownership marks, diagnostics with
quickfixes, and the extraction and implement actions. The IDE answers
with its own results first; this server's surfaces add to them.

## Requirements

- RustRover 2025.2 or newer (the open platform LSP API).
- The server binary: `cargo install rustmanifest-lsp` (installs
  `rust-manifest`, expected on `PATH`).

## Build

```sh
./gradlew buildPlugin
```

The produced zip under `build/distributions/` installs via
Settings → Plugins → Install Plugin from Disk, and is publishable to
JetBrains Marketplace.
