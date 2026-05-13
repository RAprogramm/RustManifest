# Security Policy

## Reporting a vulnerability

Report security issues privately via **GitHub Security Advisories** for this repository:

> https://github.com/RAprogramm/RustManifest/security/advisories/new

Do **not** open a public issue, discussion, or PR describing the vulnerability before it is fixed.

A maintainer will acknowledge the report within **72 hours**. We aim to ship a fix within **30 days** for high-severity issues; you will receive a CVE coordination plan with the acknowledgement.

## What qualifies

- Memory safety bugs in the engine, MCP server, or LSP server.
- Sandbox escapes from the MCP server (reading or writing outside declared roots, spawning unintended subprocesses, network egress when forbidden).
- Authentication or authorization flaws in the HTTP transport (OAuth 2.1 implementation, token validation, audience handling).
- Prompt injection paths that allow an attacker-controlled methodology resource to exfiltrate data or trigger unintended tool calls.
- Supply-chain integrity issues with our release artifacts (signing, provenance, SBOM correctness).
- Denial of service through pathological input crashing the engine or exhausting memory beyond documented bounds.

## What does not qualify

- False positives or precision regressions in rule analysis (open a regular issue).
- Wishlist items for additional rules (open a rule proposal).
- Findings in third-party crates we depend on without a clear path to exploit through `rustmanifest` (report upstream; we will coordinate).

## Supported versions

Only the latest minor of the engine and the latest minor of the rules pack receive security fixes. Older versions are out of support upon a new minor release.

## Embargo

We coordinate disclosure with the reporter and any affected downstreams. Default embargo is 30 days from acknowledgement, extendable on request. Public disclosure happens via the same GitHub Security Advisory once a fix is available.
