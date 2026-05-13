# Threat Model — rustmanifest

Initial threat model for Phase 0. Updated whenever a new component (server transport, sampling, integration) ships.

## Scope

In-scope:

- The `rustmanifest` CLI, engine, rules pack, schemas.
- The `rustmanifest` MCP server (stdio and Streamable HTTP transports).
- The `rustmanifest` LSP server.
- The GitHub Action and release pipeline.

Out-of-scope:

- The user's IDE, terminal, or shell.
- The host LLM provider serving sampling requests.
- The user's GitHub account or CI/CD secrets.

## Assets

1. **Source code being analyzed** — confidentiality and integrity.
2. **MCP/LSP server process boundary** — file-system reads/writes, subprocess spawning, network egress.
3. **Methodology resources** — markdown content embedded in the rules pack; serves as both data and instructions to the LLM, so an attacker-controlled resource is an instruction-injection vector.
4. **Release artifacts** — binary integrity, signatures, provenance, SBOM accuracy.
5. **Auth tokens** (Streamable HTTP transport) — OAuth 2.1 access tokens and refresh tokens.

## Adversaries

- **A1 — Malicious source repository**: user analyzes a repo containing crafted Rust files designed to exploit the engine (panic, OOM, file-system escape via parser bugs).
- **A2 — Malicious rules pack**: third-party pack with rules containing instruction-injection payloads in rationale text.
- **A3 — Network attacker** (HTTP transport): on-path between MCP client and server.
- **A4 — Compromised contributor**: PR with seemingly-benign code that backdoors the engine or weakens the supply chain.
- **A5 — Token thief**: leaked OAuth token from a misconfigured client.

## Threats and mitigations

### T1 — Sandbox escape via parser bug

**Adversary:** A1. **Impact:** RCE in MCP server context, exfiltration of files outside declared roots.

**Mitigations:**
- `unsafe_code = "forbid"` workspace-wide.
- Parser fuzzing (`cargo-fuzz`) gates every PR touching the engine.
- Engine never spawns subprocesses except in tier 3, which is off by default and lists allowed binaries.
- MCP server respects `roots` declared by the client; reads outside roots return an explicit error.

### T2 — Prompt injection through methodology resource

**Adversary:** A2. **Impact:** LLM following attacker instructions embedded in rationale markdown, leading to unintended tool calls or data exfiltration.

**Mitigations:**
- Default rules pack is embedded at build time from the in-tree EN methodology; no runtime fetch.
- Third-party packs are explicitly opt-in via `rustmanifest.toml`; loading a pack prints its signature, hash, and origin.
- Resources are returned as plain text without active content; client renders them, server never instructs the LLM to execute them.
- Sampling prompts (Phase 4+) sanitize methodology content before inclusion.

### T3 — Token theft and replay

**Adversary:** A3, A5. **Impact:** Unauthorized access to remote MCP server, code analysis of arbitrary uploaded files.

**Mitigations:**
- OAuth 2.1 with PKCE only; no implicit flow.
- Resource indicators (RFC 8707): tokens are audience-scoped to a specific server URL.
- Short access-token TTL (15 min); refresh tokens rotated on every use.
- TLS 1.3 only on HTTPS endpoints; no plaintext fallback.

### T4 — Supply-chain compromise

**Adversary:** A4. **Impact:** Backdoored binary published to crates.io or GitHub Releases.

**Mitigations:**
- All releases signed with sigstore keyless via GitHub OIDC.
- SLSA Level 3 build provenance attached to every artifact.
- CycloneDX SBOM published per release.
- `cargo-deny` blocks vulnerable, yanked, or duplicate dependencies.
- `cargo-audit` runs on every PR and on a daily schedule.
- New maintainers added only via signed PR by the existing maintainer set.

### T5 — Resource exhaustion

**Adversary:** A1, A3. **Impact:** Server crash, denial of service.

**Mitigations:**
- Engine has a documented per-file memory budget; files exceeding it are skipped with an explicit finding.
- HTTP transport applies per-client token-bucket rate limiting.
- Streaming for large diffs and reports; no unbounded in-memory accumulation.

### T6 — Deterministic-output bypass

**Adversary:** A1. **Impact:** False negative — attacker crafts code that the engine claims is clean by exploiting non-determinism.

**Mitigations:**
- All analyzers are required to be deterministic by contract (`Analyzer` trait docs).
- Golden tests verify identical findings across runs.
- No PRNG, no system time, no environment reads inside analyzers.

## Trust boundaries

```
+----------------+      +-----------------+      +------------------+
|  MCP / LSP     |<----| Server process   |---->| Methodology     |
|  client (IDE)  | RPC | (rustmanifest)   |     | resources       |
+----------------+      +-----------------+      +------------------+
                              |   |
                              v   v
                       +---------------+
                       | Engine + pack |
                       +---------------+
                              |
                              v
                       +---------------+
                       | Source files  |
                       | (within roots)|
                       +---------------+
```

Each arrow is a trust boundary; each is enforced by the mitigations above.

## Review cadence

- Reviewed before every minor release of the engine.
- Reviewed when a new transport, integration, or rule tier is added.
- Reviewed on every reported vulnerability (T-number assigned, document updated).
