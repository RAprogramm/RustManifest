# rustmanifest-report

Output renderers for `rustmanifest` findings:

- **JSON** — canonical machine-readable format.
- **SARIF 2.1.0** — for GitHub code scanning ingestion and other CI integrations.
- **TTY** — human-readable terminal output with color and source snippets.

Phase 0 ships only the `Renderer` trait surface; concrete renderers land in Phase 1.
