# rustmanifest-config

Loader and resolver for `rustmanifest.toml`. Handles:

- Profile selection (`strict`, `default`, `minimal`, `ci`)
- Per-rule severity overrides
- Include and exclude glob patterns
- Inline `// rustmanifest: allow(RULE-ID) reason="…"` pragmas

Phase 0 re-exports the canonical `Config` and `RuleOverride` types from `rustmanifest-schema`. Parsing, profile resolution, glob filtering, and pragma handling land in later phases.
