# rustmanifest-schema

Canonical JSON Schemas and Rust types for the `rustmanifest` ecosystem. This crate is the single source of truth for the on-the-wire shapes used by the engine, CLI, MCP server, LSP server, and any external consumer.

## Types

- `Rule` — a single review rule (id, tier, severity, title, rationale URI).
- `Finding` — output of analysis for one rule against one location.
- `Config` — `rustmanifest.toml` schema (profiles, overrides, pragmas).
- `MethodologyResource` — pointer to an embedded methodology document.

## Schema export

```
cargo run --bin rustmanifest-schema-export -- --out crates/rustmanifest-schema/schemas
```

CI regenerates schemas on every PR and fails on drift.
