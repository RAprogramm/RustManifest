# RFC 0001 — Semantic tier: fact base, SCIP frontend, workspace invariants

| Field | Value |
|-------|-------|
| Status | Draft |
| Author | @RAprogramm |
| Created | 2026-07-31 |
| Last updated | 2026-07-31 |
| Affected crates | `rustmanifest-schema`, `rustmanifest-engine`, `rustmanifest-rules-core`, `rustmanifest-cli` |
| SemVer impact | minor (new `RuleDefinition::Semantic` implementation; schema already reserves the variant) |

## Summary

Implement the `semantic` rule tier as a Datalog fact base populated from two new
frontends — a workspace-level `syn` pass and a SCIP index exported from
rust-analyzer — and ship the first workspace-topology rules (layer boundaries,
cross-crate invariants) on top of it. Every semantic finding carries its Datalog
derivation tree as a machine-readable trace, replacing message-only diagnostics.

## Motivation

The `RuleDefinition` enum has reserved a `Semantic` variant since Phase 1A, with no
design behind it. Meanwhile all shipped and planned rules (tier-1 pattern, tier-2 AST
in Phase 1C) are file-scoped: no rule can express "layer `domain` must not depend on
layer `infra`", "this error type never crosses the crate boundary", or "trait X is
implemented only in module Y". A survey of the ecosystem (July 2026, see Prior art)
shows this is a genuinely empty niche — nothing in the Rust toolbox checks project
topology — while everything file-scoped is contested by clippy, dylint, ast-grep and
semgrep. The workspace level is where this project can be unique rather than better.

A second motivation is explainability. Current findings carry a message and a
methodology URI. A Datalog engine gives the derivation tree for free: which facts,
which rule, which chain. That trace is exactly what the Phase 2 MCP server needs to
hand an LLM client a finding as a structured contract (the ZeroFalse setup, F1
0.91–0.95 published) instead of a code snippet.

### Ecosystem survey — July 2026 (what exists, what does not)

Baseline: stable Rust 1.97.1 (2026-07-16), edition 2024, releases every 6 weeks, no
LTS. Any tool linking compiler internals must pin a toolchain and update on that
cadence — the dominant operational cost for deep analysis.

**Occupied layers** (do not enter):

| Layer | Tools | Note |
|---|---|---|
| Syntactic lints | clippy (700+ lints), dylint, ast-grep, semgrep | per-crate or per-file; CodeQL has no first-class Rust |
| Deep verification | Miri, Kani, MIRAI, Prusti/Creusot/Flux, Rudra, lockbud, SafeDrop | unsafe/UB/concurrency; curated in BurtonQin/Awesome-Rust-Checker |
| Supply chain | cargo-audit / deny / auditable, RustSec | policies and known vulns |
| LLM repair | RustAssistant (~74% on compilation errors, ICSE 2025), ComBench, PanicFI | iteration LLM ↔ compiler is the proven shape |
| LLM triage | IRIS, ZeroFalse, QASecClaw (−88.6% FP) | "statics find, LLM judges" is the 2026 industry standard |
| AI review market | CodeRabbit, Greptile, Qodo, Copilot PR, Semgrep ($24–40/seat/mo) | none checks architectural invariants |

**Empty niches** (ranked; ✅ = already covered by this project):

| # | Niche | Status |
|---|---|---|
| 1 | Workspace-level architectural invariants (ArchUnit for Rust) | **this RFC** |
| 2 | Machine-checkable project rule manifest | ✅ core mission, shipped since 1B |
| 3 | Unified fact model across syntax/names/types/API | **this RFC** (foundation) |
| 4 | Findings with derivation traces | **this RFC** |
| 5 | Deterministic gate for LLM patches | ✅ Phase 2 MCP, strengthened by this RFC |
| 6 | SARIF from Rust tools | ✅ shipped in 1B |
| 7 | Compiler-error fix-maneuver catalog by E-code | Future possibilities |

**Key infrastructure facts** informing the design:

| Fact | Consequence |
|---|---|
| rust-analyzer emits SCIP; format is protobuf, typed, ~8× smaller than LSIF | name resolution without linking unstable `ra_ap_*` crates |
| `ra_ap_*` API is undocumented and breaks every release (rust-analyzer#14466 open) | never link it; SCIP is the boundary |
| `rustc_public` (ex stable-mir) offers a SemVer-compliant MIR API; Kani migrating | viable deep frontend later; not in this RFC |
| `ascent` embeds Datalog in Rust (proc-macro, semilattices, fixpoints); `ascent-interpreter` evaluates at runtime | compiled built-in rules + user rules from config without recompilation |
| `cargo-semver-checks` writes lints as Trustfall queries over rustdoc JSON; 2026 project goal: merge into cargo | reuse Trustfall for the API layer instead of inventing a schema |
| Tsetlin machine: `tsetlin-rs` exists, but weighted TM ≡ perceptron (published); clause readability degrades at scale | excluded from the roadmap; a finding-ranking experiment must beat logistic regression on identical features by ≥5pp F1 before any integration |

### Relationship to prior own work

`cargo-quality` (v0.4.0, crates.io) ships four hardcoded file-scoped analyzers
(empty lines, format args, inline comments, path imports). They are candidates for
migration into the tier-2 rules pack; `cargo-quality` stays a separate shipped tool
until that migration is decided in its own RFC. Nothing in this RFC duplicates it.

An exploratory standalone plan ("rust-arch-engine", 2026-07-31) proposed the same
engine from scratch. Comparison against this repository showed Phases 0–1B already
cover its rule format, SARIF output, clippy non-duplication policy, knowledge-base
URIs and noise governance with a hardened release pipeline on top. The standalone
repository is retired; its unique content — fact base, SCIP frontend, workspace
invariants, trace-bearing findings — is this RFC.

## Detailed design

### Fact base

A new module in `rustmanifest-engine` (or a new crate `rustmanifest-facts` if the
dependency graph warrants it) defines flat relations populated by frontends:

```text
crate_(id, name)
module(id, crate_id, path)
function(id, module_id, name, visibility)
type_def(id, module_id, name, kind)          kind: struct | enum | trait | alias
impl_of(type_id, trait_id, module_id)
calls(caller_fn, callee_fn, file, line)      resolution: scip | syntactic
depends(from_crate, to_crate, edge_kind)     edge_kind: normal | dev | build
returns_result(fn_id, ok_type, err_type)
module_layer(module_id, layer)               from rustmanifest.toml layer map
```

Facts are content-addressed per file (`file_hash`) so the Phase 1D cache design
applies unchanged: a frontend re-emits facts only for changed files.

### Frontends (two-speed)

| Frontend | Source | Latency | Produces |
|---|---|---|---|
| Workspace-syn | `syn` over all crates + `cargo metadata` | ms–s, incremental | modules, items, `use` edges, syntactic call candidates |
| SCIP | `rust-analyzer scip .` output, parsed via protobuf | seconds, on demand | resolved definitions/references → precise `calls`, `impl_of` cross-crate |

The SCIP frontend is optional at runtime: if the index is absent, semantic rules
that require resolution report `skipped (no index)` rather than degrading to wrong
answers. `calls` facts carry their resolution provenance so rules can require
`scip`-grade edges.

Deliberately out of scope: linking `ra_ap_*` (unstable), `rustc_public`/MIR (Phase 4+
material), rustdoc JSON/Trustfall (its own future RFC for API-surface rules).

### Rule evaluation

Built-in semantic rules are `ascent` programs compiled into `rustmanifest-rules-core`
next to the existing pack. User-defined semantic rules load from `rustmanifest.toml`
and evaluate via `ascent-interpreter` — no recompilation, consistent with the
Phase 1D config design. The rules-pack layout is unchanged: each semantic rule keeps
`rule.toml` + `pass.rs` / `fail.rs` fixtures; fixtures for workspace rules are
minimal multi-crate trees under the rule directory.

First shipped rules:

| ID | Invariant | Default severity |
|---|---|---|
| RM-ARCH-001 | layer dependency violation (`module_layer` + transitive `depends`/`use`) | error |
| RM-ARCH-002 | dependency cycle between workspace crates/modules | error |
| RM-ARCH-003 | trait implemented outside its designated module set | warning |
| RM-ARCH-004 | error type crossing a declared crate boundary | warning |

Layer map lives in `rustmanifest.toml`:

```toml
[semantic.layers]
domain = ["crates/app-core", "crates/app-model"]
infra  = ["crates/app-db", "crates/app-http"]

[[semantic.forbid]]
from = "domain"
to   = "infra"
```

### Traces

`Finding` gains an optional `trace` field (schema minor bump, golden JSON Schemas
regenerated through the existing drift gate): the Datalog derivation serialized as a
tree of `{relation, args, rule, children}`. The TTY renderer shows a compact chain;
JSON/SARIF carry it whole (SARIF `codeFlows`). Phase 2 MCP `explain_finding` returns
the trace plus methodology URI — the finding-as-contract shape.

### Orchestration

Semantic evaluation is a workspace-scoped stage after the per-file stages, behind the
existing cancellation token and a separate memory budget. Deterministic output
ordering is preserved (facts sorted before fixpoint, stable finding sort as today).

## Drawbacks

| Cost | Assessment |
|---|---|
| New dependencies: `ascent`, `ascent-interpreter`, `prost`/SCIP protobuf | vetted via cargo-deny; ascent is proc-macro-heavy → compile-time cost on rules-core |
| SCIP index generation requires rust-analyzer binary on PATH | optional feature; skipped-not-wrong semantics |
| Fixpoint evaluation on large workspaces | bounded by fact count, not LOC; eval corpus (Phase 1E) gets a workspace-scale benchmark with a p95 budget |
| Schema growth (`trace`) | optional field, additive, minor bump |

## Rationale and alternatives

| Alternative | Why rejected |
|---|---|
| Imperative visitors for workspace rules | transitive closure / cycles / reachability are one Datalog line each; visitor code rots and cannot emit derivation trees |
| Link `ra_ap_*` for resolution | unstable, undocumented, breaks every release; doubles maintenance for no gain over SCIP |
| CodeQL | no first-class Rust support |
| Soufflé as engine | external C++ toolchain; ascent keeps the engine pure-Rust and embeddable in the LSP/MCP servers |
| Trustfall for everything | right tool for rustdoc JSON API rules (future RFC); wrong shape for recursive graph invariants |
| Build it as a separate project | duplicates Phases 0–1B of this repository; see Motivation |

Not doing this leaves the project a better-packaged file linter in a contested
space; the workspace level is uncontested.

## Prior art

- ArchUnit (JVM), dependency-cruiser (JS) — layer rules as executable tests; no Rust equivalent exists.
- `cargo-semver-checks` — declarative queries (Trustfall) as lints; model for rule-as-query and for merging into cargo.
- Glean (Meta) — typed fact base over code, SCIP ingestion for Rust; validates the facts-first architecture at scale.
- SCIP (Sourcegraph) — protobuf index; rust-analyzer emits it natively.
- ascent / FlowLog (2025) — embedded and incremental Datalog respectively.
- ZeroFalse / IRIS / QASecClaw (2025–2026) — findings as structured contracts adjudicated by an LLM; the trace format here is designed to feed that pipeline via Phase 2/4.
- RustAssistant (ICSE 2025) — ~74% compile-error fix accuracy via LLM–compiler iteration; baseline for the maneuver-catalog future work.
- Rudra, lockbud, SafeDrop — deep unsafe/concurrency checkers; complementary, not overlapping.

## Unresolved questions

1. Fact base placement: module inside `rustmanifest-engine` or new `rustmanifest-facts` crate? (Decide by dependency direction with rules-core.)
2. SCIP invocation: shell out to `rust-analyzer scip` or vendor the protobuf schema and require a pre-built index path in config?
3. Trace size limits in SARIF for pathological derivations — truncation policy and its pragma story.
4. Does `RM-ARCH-004` need `returns_result` facts from signatures only, or body-level `?`-propagation (which would pull tier-3/MIR forward)?
5. Multi-crate fixtures: does the rules-core build script need workspace-aware fixture compilation, and what does that do to build time?

## Future possibilities

- rustdoc JSON + Trustfall API-surface rules (public-API size ties into STRUCTURE.md §4 at workspace scale).
- `rustc_public`/MIR deep frontend: error/panic flow facts, borrow-shaped facts.
- Compiler-error maneuver catalog keyed by E-code over `cargo check --message-format=json`, ranked by facts, instantiated via MCP sampling; RustAssistant's 74% is the bar.
- Finding-ranking experiment (logistic regression baseline; any fancier model must beat it by ≥5pp F1 on identical binary features).
- `cargo-quality` analyzer migration into tier-2 pack.

## Implementation plan

Each step is a separate PR, gated by the previous one being green; roadmap gains a
phase entry ("Phase 1G — semantic tier") referencing this RFC.

| Step | Content | Gate |
|---|---|---|
| 1 | Schema: `trace` field + semantic config section; golden schemas regenerated | drift gate green |
| 2 | Fact base + workspace-syn frontend; facts snapshot-tested on a fixture workspace | deterministic fact dump |
| 3 | ascent evaluation + RM-ARCH-002 (cycles, needs no layer config) | pass/fail fixtures |
| 4 | Layer config + RM-ARCH-001; TTY/JSON/SARIF trace rendering | fixtures + snapshot tests |
| 5 | SCIP frontend (optional feature) + RM-ARCH-003/004 upgraded to `scip`-grade edges | skipped-without-index semantics tested |
| 6 | Workspace-scale benchmark in eval corpus; p95 budget recorded | Phase 1E regression gate extended |

Owner: @RAprogramm (sole maintainer). Phase 1C (AST tier-2) proceeds first as
planned; this RFC lands as 1G to avoid blocking the shipped roadmap order.
