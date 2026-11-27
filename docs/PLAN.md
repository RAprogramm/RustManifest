# Rust Architecture Engine — Plan & State of the Art (July 2026)

> Status note (2026-11-27): this is a historical research and planning document.
> Current implementation status, shipped phases, and active priorities are tracked
> in [`ROADMAP.md`](./ROADMAP.md).

Deterministic analyzer for Rust workspaces: rules and a semantic fact base make the
decisions, an LLM only proposes candidate changes and explains tasks. Every finding
carries a full derivation trace ("why").

---

## Part 0. Core plan (original)

### Phase 1 — MVP core

| Module | Responsibility |
|---|---|
| Indexer | `syn` over the whole workspace → AST + symbol table (functions, types, traits, modules, impl blocks) |
| Semantics | graphs: calls, dependencies, trait impls, error flows (`Result`/`?`), ownership at signature level |
| Rule engine | rule = predicate over AST/graph → finding with severity, location, trace |
| Rule DSL | declarative pattern description so rules are added without rebuilding logic |
| Explanation | per finding: which rule, which nodes matched, what exactly is violated |

### Phase 2 — learnable part

Tsetlin machine over binary features extracted by the rule engine: a classifier over
already-computed facts, not "a network looking at code". Yields interpretable clauses.
Prototyped separately on a labeled findings set — must beat a simple threshold rule
before integration.

### Phase 3 — compiler errors

Parse `cargo check --message-format=json`, bind error codes (E0382, E0499, E0308…)
to AST nodes, build a fix strategy from a catalog of typed maneuvers (clone,
borrow-split, lifetime, type cast) — not whole-file regeneration.

### Phase 4 — knowledge base

Rust Book, Cargo Book, RFCs, crate docs, project manifest — an indexed corpus
isolated from the model. Source of explanations and citations, never of decisions.

### Phase 5 — LLM layer

Input: human task description. Output: change candidates. Every candidate must pass
the rule engine and `cargo check`; failures are dropped. The model never merges.

### Initial risks

1. **Clippy duplication** — 80% of simple rules already exist. Value only in
   cross-module and architectural checks clippy does not do. Confirm on a real repo
   before starting.
2. **`syn` vs rust-analyzer** — `syn` gives no name/type resolution; call graph will
   be approximate without deeper tooling.
3. **Tsetlin** — the most debatable part. Deploy only after rules already produce
   findings, otherwise there is nothing to train on.

Implementation order: 1 → 3 → 4 → 5 → 2 (phase 2 deliberately last).

---

## Part 1. What exists as of July 2026

### 1.1 Baseline

| Fact | Value |
|---|---|
| Stable Rust | 1.97.1 (July 16, 2026), releases every 6 weeks |
| Current edition | 2024 (since 1.85, Feb 2025); next expected 2027 |
| LTS | none; only latest stable gets patches |

Any tool touching compiler internals must pin a toolchain version and update every
6 weeks. This is the project's main operational cost.

### 1.2 Layer 1 — syntactic linters (densely occupied)

| Tool | Provides | Limitation |
|---|---|---|
| clippy | 700+ lints: correctness, perf, style, complexity, restriction; ships with toolchain | per-crate, no workspace view, hardcoded rules |
| dylint (Trail of Bits) | custom lints as dynamic libraries, `EarlyLintPass`/`LateLintPass`, `clippy_utils` access, builds drivers per compiler version | written in Rust against unstable `rustc_*` API |
| ast-grep | tree-sitter, code-like patterns with `$METAVAR`, Rust support, multicore | pure syntax, zero types |
| semgrep | Rust among 30+ languages, SARIF output | syntactic level for Rust, no name resolution |
| CodeQL | deep semantics | **no first-class Rust support** |

Conclusion: anything at the "indentation, naming, simple antipattern" level is already
done and free. No point entering this layer.

### 1.3 Layer 2 — semantics and indexing

| Technology | State |
|---|---|
| `ra_ap_*` (rust-analyzer as a library) | published on crates.io; `Analysis` snapshot over a salsa DB and the `hir` crate. Unstable API, near-zero docs; "how to use as a crate" issue (rust-analyzer#14466) still open |
| SCIP (Sourcegraph) | replaced LSIF; protobuf, ~8× smaller, ~3× faster; rust-analyzer emits SCIP; Glean supports SCIP for Rust |
| `rustc_public` (ex project-stable-mir) | SemVer-compliant public API over MIR; Kani migrating to it; `rustc_public_bridge` with `internal()`/`stable()` |
| Compiler plugins | **do not exist and will not**. Only path: own binary embedding rustc, overriding queries (`optimized_mir` most useful). Framework: `rustc_plugin` (cognitive-engineering-lab) |
| rustdoc JSON + Trustfall | `cargo-semver-checks` writes rules as declarative typed queries over the rustdoc JSON schema; engine abstracts format versions. 2026 Rust project goal: merge into cargo itself |

### 1.4 Layer 3 — verification and bug hunting

| Tool | Method | Catches |
|---|---|---|
| Miri | MIR interpretation | UB during test runs |
| Kani | bit-precise model checking | property proofs, esp. `unsafe` |
| MIRAI | abstract interpretation of MIR | panics, security, correctness |
| Prusti / Creusot / Flux | Viper / WhyML / refinement types | formal contracts |
| Rudra | unsafe dataflow + Send/Sync variance | scanned 43k crates in 6.5h; 264 unknown bugs, 76 CVEs, 112 RustSec advisories |
| lockbud (TSE'24) | static | double-lock, lock-order conflicts, atomicity violations, UAF, invalid free, panic sites |
| SafeDrop | path-sensitive dataflow, modified Tarjan | invalid memory deallocation |
| cargo-audit / deny / auditable, RustSec | supply chain | known vulns, policies |

Curated list: `BurtonQin/Awesome-Rust-Checker`. Walk it entirely before starting and
cross out everything already done.

### 1.5 Layer 4 — LLM: what is already proven

| Work | Result |
|---|---|
| RustAssistant (Microsoft, ICSE 2025) | fixing Rust compilation errors, ~74% peak accuracy on real repos; key = LLM ↔ compiler iteration, not one-shot prompting |
| ComBench (2026) | repo-level compilation-error-repair benchmark |
| RustMizan (2026) | compilable, contamination-aware Rust vulnerability benchmark |
| PanicFI | infrastructure for fixing panic bugs in real Rust programs |
| AdaTrans (2026) | C→Rust via error-adaptive repair |
| IRIS | LLM + CodeQL, whole-repo reasoning; −5pp FDR, +28 vulns found |
| ZeroFalse | static findings as "structured contracts" + LLM judge; F1 0.912 (OWASP Java), 0.955 (OpenVuln) |
| QASecClaw | multi-agent FP filtering: 560 → 64 (−88.6%) |

**Critical**: the scheme "deterministic statics find → LLM only explains and judges,
never decides" is not a hypothesis anymore — it is the 2026 industry standard with
published numbers. The architectural decision is correct but not a differentiator.
Differentiation must come from elsewhere.

### 1.6 AI review market (the comparison set)

| Product | Positioning | Price (June 2026) |
|---|---|---|
| CodeRabbit | fewer findings, near-zero noise | ~$24/dev/mo |
| Greptile | whole-repo indexing, 82% catch rate, most false positives | ~$30/seat |
| Qodo | review + test generation, multi-agent | ~$30/user |
| Copilot PR review | zero setup; context = diff only, systematically misses cross-file bugs | in GH Enterprise |
| Semgrep | security leader | ~$40/mo |

### 1.7 Tsetlin — sober picture

| Aspect | State |
|---|---|
| Rust implementation | `tsetlin-rs` crate (v0.3.4): lock-free parallel training, bitwise clauses, `no_std`, SIMD, const generics; claimed 25–92× clause evaluation speedup |
| Reference | `cair/pyTsetlinMachine`, `cair/tmu`; CUDA-only stack (NVIDIA); Metal port appeared June 2026 |
| Community | ISTM 2026 conference; Probabilistic TM (uncertainty quantification), Convolutional TM |
| **Counterargument** | published equivalence of weighted Tsetlin machine and the perceptron; clause interpretability degrades with clause count, needs pruning |

### 1.8 Agent tooling

MCP servers over rust-analyzer already exist (`zeenix/rust-analyzer-mcp`,
`rust-analyzer-mcp` crate, universal `mcpls` MCP↔LSP bridge, `zircote/rust-lsp`
plugin). 2026 consensus on code search for agents: hybrid of three tools — lexical +
structural + graph — with mandatory verification against disk. Fresh paper: "Code
Isn't Memory: A Structural Codebase Index Inside a Coding Agent".

---

## Part 2. What does NOT exist — real niches

Sorted by descending value.

| # | Niche | Why it is empty |
|---|---|---|
| 1 | **Workspace-level architectural invariants** — ArchUnit/dependency-cruiser for Rust: "layer A must not import layer B", "trait X implemented only in module Y", "this error type never crosses the crate boundary" | clippy is per-crate; cargo-deny is about external deps, not internal layers; nothing checks project topology |
| 2 | **Machine-checkable project rule manifest** — own conventions as executable spec (e.g. "no `//` comments outside `#[cfg(test)] mod tests`") | dylint allows it but requires Rust against unstable rustc API — entry barrier excludes 99% of teams; no declarative DSL exists |
| 3 | **Unified fact model across all levels** — syntax + resolved names + types + MIR + rustdoc JSON in one queryable base | every tool builds its own frontend from scratch, nothing is reused |
| 4 | **Finding derivation trace** — not a message string but a tree: which facts, which rule, which chain | clippy gives a message; CodeQL gives paths but does not support Rust |
| 5 | **Deterministic gate for LLM patches** — agent generates; only what passes project rules + `cargo check` + tests gets merged | agents check compilation; nobody checks architectural invariants |
| 6 | **SARIF from Rust tools** | standard exists; Rust ecosystem barely emits it; no GH code-scanning integration |
| 7 | **Compiler-error fix-strategy catalog** by error code (E0382, E0499, E0308, E0507…) as explicit maneuvers | RustAssistant hit 74% but as an LLM loop, not an inspectable catalog; no reproducibility or explainability |

Niches 1, 2, 5 are the strongest. Niche 3 is the foundation under them. The rest is
derivative.

---

## Part 3. How to build it professionally

### 3.1 Positioning (decide before the first line of code)

Not "another Rust analyzer". Formula: **an executable specification of project
architecture that simultaneously gates humans and LLM agents.** Everything clippy
already does is integrated, not reimplemented.

Hard rule: before adding any rule — grep the clippy lint list. Duplicate → rejected.
Keep an overlap table in the repo.

### 3.2 Core architecture — two-speed frontend

| Level | Technology | Latency | Available |
|---|---|---|---|
| Fast | `syn` + tree-sitter/ast-grep | ms, per file save | syntax, module structure, attributes, macro calls as text |
| Medium | SCIP index exported from rust-analyzer | seconds, incremental | name resolution, defs, refs, impls, cross-crate |
| Deep | own driver over `rustc_public`, dylint/`rustc_plugin` style | minutes, on CI | types, MIR, dataflow, borrow info |
| API | rustdoc JSON | seconds | public surface, semver |

Why SCIP instead of linking `ra_ap_*`: rust-analyzer already emits SCIP; the format
is stable and typed (protobuf), while `ra_ap_*` is an undocumented unstable API
breaking every release. Linking it doubles maintenance cost with no gain. This is
the single most important engineering decision in the project.

### 3.3 Rule engine — Datalog, not imperative visitors

All four frontends normalize into a flat fact base: `function(id, name, module)`,
`calls(caller, callee, site)`, `implements(type, trait)`, `returns_result(fn, err_ty)`,
`module_layer(mod, layer)`, etc. Rules are Datalog. Engine: `ascent` (embedded DSL,
proc-macro compiled) + `ascent-interpreter` for rules loaded at runtime from project
config. Souffle only if heavy batch is ever needed (drags in C++).

| Property | Mechanism |
|---|---|
| Finding explanation | the Datalog derivation tree **is** the "why" trace; no separate explanation module |
| Recursive checks | transitive closures (call-graph reachability, dependency cycles) — one line instead of a visitor |
| Abstract interpretation | semilattice types in ascent, fixpoint iteration is the native pattern |
| User-facing DSL | project rule = declarative query, not Rust code with `LateLintPass` |
| Incrementality | salsa-like caching by content hash; see FlowLog (2025) on incremental Datalog |

Alternative for API rules — Trustfall, as in `cargo-semver-checks`: typed queries over
a schema, datasource-agnostic. Use it for the rustdoc JSON layer instead of inventing
our own.

### 3.4 Rule format

Every rule is a file with mandatory fields:

| Field | Purpose |
|---|---|
| stable id | PR references, suppressions, history |
| category + severity | CI filtering |
| frontend level | makes execution cost explicit |
| query | Datalog/Trustfall |
| message with substitutions | human-readable |
| rationale + knowledge-base link | Rust Book / RFC / Nomicon / project manifest |
| negative test | code that must **not** trigger |
| clippy overlap | explicit `none` or lint id |
| autofix | optional, only if deterministic |

Rule testing — UI tests clippy-style (`ui_test`): fixture + expected output, diff
breaks CI. Without this the rule collection rots in three months.

### 3.5 Compiler errors

Do not invent a parser. `cargo check --message-format=json` already yields structured
diagnostics with spans and machine-applicable suggestions (what `rustfix` lives on).
On top:

1. Bind diagnostic to a fact-base node (span → fact id).
2. Maneuver catalog per error code: E0382 (use after move) → {clone, borrow,
   restructure ownership, `Rc`}; E0499 (two mutable borrows) → {borrow split,
   `split_at_mut`, reorder, interior mutability}; E0308 → {cast, `From`/`Into`,
   fix signature}.
3. Rank maneuvers by context using facts, not the model.
4. LLM picks and instantiates a maneuver; compiler verifies; loop repeats.

Baseline to beat: RustAssistant, ~74%. Below that — not ready.

### 3.6 LLM layer: the contract

| Rule | Statement |
|---|---|
| Input | natural-language task + relevant fact-base slice (not raw files) |
| Output | patch candidates, each separate |
| Gate | `cargo check` → `cargo clippy -D warnings` → project rules → tests. Fail → silently dropped |
| Forbidden | model never merges, never suppresses findings, never edits rules |
| Judge role | allowed only for FP suppression, ZeroFalse-style: finding passed as a structured contract with trace, not as a code snippet |

The "structured contract + trace + LLM judge" scheme yields published F1 ~0.91–0.95
and up to −88% FP. Copy this setup directly.

### 3.7 Delivery surfaces

One core, four outputs — what separates a professional tool from a script:

| Surface | Form |
|---|---|
| CLI | `cargo` subcommand, SARIF 2.1.0 + own JSON |
| CI | GitHub code scanning via SARIF, PR annotations, per-PR noise budget |
| Editor | LSP server running on the fast frontend |
| Agent | MCP server: fact-base queries, patch validation, finding explanation |

The MCP surface is not decoration — it IS niche #5: the agent gets a deterministic
oracle instead of guessing.

### 3.8 Where the Tsetlin machine goes

Honestly: **the weakest part of the concept; must not be foundational.** Arguments
against: published weighted-TM ≡ perceptron equivalence; clause readability loss at
scale.

The only justified place: ranking and suppression of findings over **binary features
already computed by rules** (rule R fired, function in layer L, has `unsafe`,
cross-crate call, covered by tests, changed in this PR…). Training data — own
accept/reject decisions on findings. Then clauses read as "rule R in core layer
without tests = accept", which is genuinely explainable.

Admission condition: **beats logistic regression on the same features by ≥ 5pp F1.**
If not — use the regression and stop. Prototype in two days on `tsetlin-rs`, before
any integration.

### 3.9 Knowledge base

Isolated, versioned corpus: Rust Book, Nomicon, Cargo Book, RFCs, dependency
rustdocs, RustSec advisories, project manifest. Indexed separately, versioned with
the toolchain. Used for explanations and citations in findings. **Never a source of
decisions** — no rule fires because "the book says so".

### 3.10 Metrics — how to judge success

| Metric | Target |
|---|---|
| Precision on own repos | ≥ 0.9; recall on external benchmarks secondary |
| PR noise | ≤ 3 findings per average PR, otherwise the tool gets disabled |
| Cold run, medium workspace | ≤ 2 min on CI |
| Incremental run | ≤ 200 ms per file save |
| Findings with full trace | 100%, no exceptions |
| Rules duplicating clippy | 0 |

### 3.11 Work order

| Stage | Content | Exit gate |
|---|---|---|
| 0 | Walk Awesome-Rust-Checker fully, build "already exists" table. Take 3 real repos, hand-collect 20 violations clippy does not catch | if 20 cannot be collected — no project, stop |
| 1 | Fast frontend (`syn`) → fact base → ascent → 10 rules → SARIF → CLI | the 20 findings are caught, FP < 10% |
| 2 | SCIP index from rust-analyzer; cross-crate and architectural rules; project layers in config | works on a 20+ crate workspace |
| 3 | LSP + MCP surfaces | an agent actually uses it as an oracle |
| 4 | Deep frontend on `rustc_public` (types, MIR), error/panic flows | pinned toolchain, ≤ 1 day per release to update |
| 5 | Compiler errors: maneuver catalog + compiler loop | > 74% on own set |
| 6 | LLM judge for FP suppression, ZeroFalse-style | precision up, recall not down |
| 7 | Tsetlin, only if condition 3.8 holds | otherwise not built at all |

### 3.12 Main risks

| Risk | Mitigation |
|---|---|
| Clippy duplication | stage-0 gate: 20 real findings by hand, before code |
| rustc frontend maintenance cost | deferred to stage 4; everything before runs on stable APIs |
| `ra_ap_*` instability | never link it; SCIP as the boundary |
| Noise kills adoption | noise budget as a hard metric from day one |
| Tsetlin eats months | 2-day prototype, explicit admission threshold |
| Macros | syntactic frontend cannot see through them; either `cargo expand` or an honestly documented limitation |
| Rule collection rot | UI tests + negative test mandatory per rule |

---

**One-sentence summary**: the strong part is a Datalog fact base over the workspace
with derivation traces and the project manifest as an executable specification that
simultaneously gates the agent; the weak part is Tsetlin. Order: 0 → 1 → 2 → 3 → 5 →
4 → 6 → 7.

---

## Sources

- [Awesome-Rust-Checker](https://github.com/BurtonQin/Awesome-Rust-Checker)
- [Dylint](https://github.com/trailofbits/dylint) · [Write Rust lints without forking Clippy](https://blog.trailofbits.com/2021/11/09/write-rust-lints-without-forking-clippy/)
- [rustc_public (Stable MIR)](https://github.com/rust-lang/rustc_public) · [Migrating to StableMIR — Kani](https://model-checking.github.io/kani/stable-mir.html) · [rustc_plugin](https://github.com/cognitive-engineering-lab/rustc_plugin)
- [ra_ap_ide](https://docs.rs/ra_ap_ide) · [rust-analyzer architecture](https://rust-analyzer.github.io/book/contributing/architecture.html) · [issue #14466](https://github.com/rust-lang/rust-analyzer/issues/14466)
- [SCIP](https://scip-code.org/) · [Announcing SCIP — Sourcegraph](https://sourcegraph.com/blog/announcing-scip) · [Glean](https://glean.software/)
- [Semgrep vs CodeQL 2026](https://appsecsanta.com/sast-tools/semgrep-vs-codeql) · [ast-grep tool comparison](https://ast-grep.github.io/advanced/tool-comparison.html)
- [ascent](https://docs.rs/ascent/latest/ascent/) · [ascent-interpreter](https://crates.io/crates/ascent-interpreter) · [FlowLog](https://arxiv.org/html/2511.00865v1) · [Soufflé](https://souffle-lang.github.io/program)
- [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) · [Rust Project Goals 2026](https://rust-lang.github.io/rust-project-goals/2026/cargo-semver-checks.html)
- [RustAssistant (ICSE 2025)](https://www.microsoft.com/en-us/research/wp-content/uploads/2024/08/paper.pdf) · [RustMizan](https://arxiv.org/pdf/2607.04729) · [PanicFI](https://arxiv.org/pdf/2408.03262) · [AwesomeLLM4APR](https://github.com/iSEngLab/AwesomeLLM4APR)
- [IRIS](https://openreview.net/pdf?id=9LdJDU7E91) · [ZeroFalse](https://arxiv.org/html/2510.02534) · [QASecClaw](https://arxiv.org/html/2605.01885v1)
- [Rudra](https://dl.acm.org/doi/10.1145/3477132.3483570) · [SafeDrop](https://dl.acm.org/doi/full/10.1145/3542948)
- [tsetlin-rs](https://crates.io/crates/tsetlin-rs) · [ISTM 2026](https://istm.no/expanded-program/) · [Weighted TM ≡ Perceptron](https://arxiv.org/pdf/2212.13634)
- [rust-analyzer-mcp](https://github.com/zeenix/rust-analyzer-mcp) · [mcpls](https://github.com/bug-ops/mcpls)
- [Rust Versions](https://releases.rs/) · [Rust releases](https://blog.rust-lang.org/releases/latest/)
- [AI code review tools 2026](https://www.stork.ai/blog/best-ai-code-review-tools-2026) · [Code Isn't Memory](https://arxiv.org/pdf/2606.22417)
