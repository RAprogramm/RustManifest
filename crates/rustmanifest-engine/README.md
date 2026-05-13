# rustmanifest-engine

Tiered analysis engine for `rustmanifest`. Implements three tiers selectable per-rule:

- **Pattern** — fast regex / aho-corasick scan over file text.
- **AST** — `syn`-based traversal for structural and local semantic checks.
- **Semantic** — `rust-analyzer` and `cargo` integrations; off by default.

The engine is rule-pack agnostic — it accepts any pack conforming to `rustmanifest-schema`. Phase 0 ships only the trait surface; concrete analyzers land in Phase 1.
