# Roadmap

Canonical plan for `rustmanifest`. Updated with every phase-completing PR.

**Current status (2026-05-13):** Phase 1B shipped. The CLI `rustmanifest check` runs end-to-end on Rust source trees with tier-1 pattern rules (security: hardcoded credentials, SQL injection, command injection). Engine, three renderers (JSON / SARIF 2.1.0 / TTY), and parallel orchestrator with cancellation and memory budget are in `main`.

**Next:** Phase 1C — AST tier-2 analyzer driven by `STRUCTURE.md`.

---

## Project goal

Ship a production-grade Rust review engine that:

1. Enforces the methodology in this repository (`README.md`, `STRUCTURE.md`, `code-review-methodology/en/`) as executable rules rather than prose.
2. Exposes those rules to AI-assisted review through MCP — every finding carries a `rustmanifest://` URI pointing to the relevant methodology section, so LLM clients can fetch the rationale on demand.
3. Does **not** duplicate `clippy`. Where clippy is strictly better (e.g. `unwrap_used`, `panic`), the strategy is tier-3 delegation, not reimplementation.

The MCP server in Phase 2 is the project's primary deliverable. Everything before it is foundation.

---

## Phases

### Phase 0 — Foundation ✅ done (#2, PR #3)

- Cargo workspace with 8 crates.
- Hardened CI on 4 OS (lint, test, MSRV 1.95, audit, deny, reuse, schema-drift, coverage, mcp-conformance scaffold).
- Tag-triggered release pipeline with sigstore signing, SLSA L3 provenance, CycloneDX SBOM.
- Schema crate (`Rule`, `Finding`, `Config`, `MethodologyResource`) with committed golden JSON Schemas and a drift gate.
- Governance: threat model, governance doc, RFC template, security policy, issue and PR templates.
- License: MIT with SPDX REUSE 3.x compliance, `reuse lint` in CI.

### Hotfix — release-plz to manual ✅ done (#5, PR #6)

- `.github/workflows/release-plz.yml` switched to `workflow_dispatch` only. No automatic publishing until the user explicitly enables it.

### Phase 1A — Rules pack format ✅ done (#4, PR #7)

- `Rule` schema reshaped: `RuleDefinition` externally tagged enum (`pattern` / `ast` / `semantic`) replaces the standalone `Tier`.
- Per-rule directory layout under `crates/rustmanifest-rules-core/rules/<RULE-ID>/{rule.toml, pass.rs, fail.rs}`.
- Build script bundles the pack into `rules.json` at compile time; `lib.rs` exposes `RULES: LazyLock<Vec<Rule>>`.
- Pack-level invariants verified by integration tests.

### Phase 1B — Tier-1 engine, renderers, CLI ✅ done (#8, PR #9)

- `PatternAnalyzer` implementing the `Analyzer` trait for tier-1 (regex) rules with `exclude_globs` honoring.
- `Orchestrator` with rayon-parallel execution, deterministic output sorting, cancellation token, per-file memory budget.
- `walker::discover` — `ignore`-crate-based gitignore-aware file enumeration.
- Inline pragma suppression: `// rustmanifest: allow(RM-RULE-ID) reason="..."` with mandatory reason.
- Three renderers: `JsonRenderer`, `SarifRenderer` (SARIF 2.1.0), `TtyRenderer` (anstyle, NO_COLOR-aware).
- CLI `rustmanifest check` subcommand with `--format auto|json|sarif|tty`, `--severity-filter`, `--no-color`, `--max-file-size`, `--verbose`.
- Exit codes: 0 clean, 1 findings present, 2 operational error.
- Tracing instrumentation, OpenTelemetry-friendly span fields.
- Coverage: 11 engine unit tests, 3 engine integration tests, 4 renderer snapshot tests, 6 CLI integration tests via `assert_cmd`, criterion benchmark scaffold.
- Decision recorded in memory: rules pack does NOT duplicate clippy. `RM-RUST-001` (panic in production) and `RM-PERF-001` (`Vec::new()` without `with_capacity`) were removed; the default pack now is `RM-SEC-001/002/003`.

### Phase 1C — Tier-2 AST analyzer ⏳ planned

- `AstAnalyzer` implementing `Analyzer` for `RuleDefinition::Ast { check }`.
- Built-in AST checks for the rules from `STRUCTURE.md`:
  - Entity naming (`STRUCTURE.md §1`).
  - Method naming (§2).
  - Structure size threshold (§3).
  - Public API surface (§4).
  - Constructor design (§5).
  - Delegating constructors (§6).
  - Immutability-first (§7).
  - Constant encapsulation (§8).
  - Fakes-only testing (§9).
- Fixture coverage: each AST check ships pass/fail fixtures wired into the existing rules-core build pipeline.
- Per-check tracing spans and memory budget integration.
- Acceptance: every section of `STRUCTURE.md` corresponds to at least one shipped rule.

### Phase 1D — Config, explain, cache ⏳ planned

- `rustmanifest-config`: parse `rustmanifest.toml` (profile selection, per-rule severity overrides, include/exclude globs).
- Built-in profiles: `strict`, `default`, `minimal`, `ci`.
- `rustmanifest explain <RULE-ID>` — prints the rule rationale and methodology section to stdout.
- Inline unused-pragma finding (`RM-META-001`): pragma that references a rule that did not fire becomes a finding itself.
- Optional persistent cache (content-addressed by `(file_hash, rule_id_set)`). Behind `--cache <dir>` flag, default off.

### Phase 1E — Eval corpus and precision metrics ⏳ planned

- `eval/` directory with 3–5 public Rust repositories as fixtures.
- Per-rule precision / recall measurement against curated labels.
- Baseline numbers committed; regression gate in CI for any drop > 2 percentage points.
- Performance budget enforcement: `p95 review_file < 100ms` for files < 1k LOC.

### Phase 1F — Reserved ⏳ planned

- Reserved for cache layer landing if not absorbed by Phase 1D.

### Phase 2 — MCP server (stdio) ⏳ planned

- `rustmanifest-mcp` built on the `rmcp` SDK.
- **Tools**: `review_file`, `review_diff`, `review_pr`, `check_rule(id, path)`, `lint_structure`, `explain_finding(finding_id)`.
- **Resources**: `rustmanifest://methodology/{section}` for every English methodology document.
- **Prompts**: `pr-review`, `pre-commit-check`, `security-audit`, `structural-audit`.
- **Roots**: client-supplied workspace; server refuses access outside.
- Conformance test suite against the Anthropic mcp-validator wired into CI.
- All server-facing strings are English regardless of request language.

### Phase 3 — Streamable HTTP, OAuth, clippy delegation ⏳ planned

- HTTP transport with OAuth 2.1 + PKCE + RFC 8707 resource indicators.
- Audience-scoped tokens, TLS 1.3 only, per-client rate limiting.
- Tier-3 delegation: `cargo clippy --message-format=json` parsed and surfaced under rustmanifest's reporting umbrella with rationale URIs.
- Optional `cargo audit` and `cargo geiger` integrations.

### Phase 4 — Sampling and elicitation ⏳ planned

- MCP `sampling` API for LLM-assisted classification of borderline findings.
- `elicitation` flow for interactive suggested-fix application.
- Per-prompt eval suite with golden labels; pass-rate gate before merge of any prompt change.

### Phase 5 — Integrations and editors ⏳ planned

- `rust-manifest/rustmanifest-action@v1` GitHub Action (composite): runs `rustmanifest check`, uploads SARIF, comments on PR.
- Pre-commit hook YAML snippet.
- `rustmanifest-lsp` — LSP server reusing the engine. Diagnostics, code actions, hover.
- Editor docs: Claude Code, Cursor, Zed, Helix, VS Code (LSP and MCP).

### Phase 6 — Governance and community ⏳ planned

- RFC process activated (template already shipped, no merged RFC yet).
- Maintainer additions (currently solo).
- Deprecation policy enforcement in CI.
- Public release readiness gate (signed binaries, attested SBOM, crates.io publishing).
- Triggers the flip of `release-plz.yml` back from `workflow_dispatch` to `push: branches: [main]`.

---

## Versioning, governance, security

These live alongside the roadmap and are referenced by every phase:

- [`docs/GOVERNANCE.md`](./GOVERNANCE.md) — SemVer per crate, RFC process, deprecation policy, severity governance, release cadence.
- [`docs/THREAT_MODEL.md`](./THREAT_MODEL.md) — adversary list, mitigations, trust boundaries.
- [`SECURITY.md`](../SECURITY.md) — private vulnerability disclosure procedure.
- [`docs/RFCS/0000-template.md`](./RFCS/0000-template.md) — RFC template.

---

## How to read this document

- **`✅ done`** — shipped in `main`. Issue closed, PR merged.
- **`⏳ in progress`** — there is an open PR or open issue actively being worked on.
- **`⏳ planned`** — defined scope; no PR yet.

Every phase that ships **must** update this file in the same PR. The PR description starts with `Updates docs/ROADMAP.md status to ...`.

This document is the source of truth; pinned tracker issues and PR descriptions reference it, never the other way around.
