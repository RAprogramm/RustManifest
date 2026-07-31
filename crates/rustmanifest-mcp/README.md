# rustmanifest-mcp

MCP server exposing the `rustmanifest` engine as tools and resources over stdio.

## Transport

- **stdio** — local IDE and CLI integration (shipped).
- **Streamable HTTP** — remote team deployments with OAuth 2.1 + PKCE + RFC 8707 resource indicators (planned, Phase 3).

## Tools

| Tool | Parameters | Result |
| :- | :- | :- |
| `review_file` | `path` | Findings from every bundled rule against one file. |
| `review_path` | `path` | Findings from every bundled rule against a file or directory. |
| `check_rule` | `rule_id`, `path` | Findings from exactly one rule; unknown ids return a descriptive error. |
| `lint_structure` | `path` | Findings from the structural rules (`RM-STRUCT-*`) only. |
| `explain_rule` | `rule_id` | Rule id, title, severity, tier, rationale URI, and the resolved methodology excerpt. |

Findings are returned as structured JSON arrays using the canonical `Finding` shape from `rustmanifest-schema`.

## Resources

The English methodology documents are embedded into the binary at compile time and served under `rustmanifest://methodology/` URIs (`structure`, `security-vulnerabilities`, `code-quality`, `rust-specific`, `performance-issues`, `examples`, `quick-reference`, `readme`, `index`). Rule rationale URIs such as `rustmanifest://methodology/structure#entity-naming` resolve to the same documents.

## Workspace confinement

The workspace root is the server's current directory at startup. Every path parameter is joined to that root and canonicalized; a canonical result outside the root is rejected with `path escapes workspace root`. The server never reads outside the workspace and performs no network access.

## Running from an MCP client

Build the binary and register it with the workspace directory as `cwd`:

```json
{
  "mcpServers": {
    "rustmanifest": {
      "command": "cargo",
      "args": ["run", "--release", "-p", "rustmanifest-mcp"]
    }
  }
}
```

All server-facing strings are English regardless of request language. The HTTP transport, prompts, and diff/PR review tools land in later phases.
