# rustmanifest-mcp

MCP server exposing the `rustmanifest` engine as tools, resources, and prompts.

Transports:

- **stdio** — local IDE and CLI integration (default).
- **Streamable HTTP** — remote team deployments with OAuth 2.1 + PKCE + RFC 8707 resource indicators.

Surface:

- **Tools** — `review_file`, `review_diff`, `review_pr`, `check_rule`, `lint_structure`, `explain_finding`.
- **Resources** — embedded EN methodology documents under the `rustmanifest://` URI scheme.
- **Prompts** — `pr-review`, `pre-commit-check`, `security-audit`, `structural-audit`.

All server-facing strings are English regardless of request language. Phase 0 ships the crate skeleton only; the stdio server lands in Phase 2 and the HTTP transport in Phase 3.
