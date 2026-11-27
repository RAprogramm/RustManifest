# Roadmap

Canonical plan for `rustmanifest`. Updated with every phase-completing PR.

**Current status (2026-11-27):** Phases 1C, 1D, 1E, 2, 2S, 2M and 2V shipped. The rules pack holds 25 rules — `RM-SEC-001`..`003`, `RM-STRUCT-001`..`011`, `RM-MEM-001`..`003`, `RM-IO-001`..`004`, `RM-QUAL-001`..`004` — and tier-2 AST rules cover every `STRUCTURE.md` section. Phase 2S (semantic tier, per [RFC 0001](./RFCS/0001-semantic-tier-fact-base.md) and [RESEARCH-2026-07](./RESEARCH-2026-07-behavioral-tier.md)): workspace type graph with derive/macro expansion absorption, module graph with cycle detection, behavioral facts (reads/writes/constructs/errors/effects), user-declared invariants, ownership facts, call graph with transitive effects. The stdio MCP server ships eight tools — `review_file`, `review_path`, `check_rule`, `lint_structure`, `explain_rule`, `project_overview`, `function_facts`, `learn_conventions` — and nine bundled methodology resources, so agents read the same project understanding the CLI prints. The CLI ships nine commands: `check`, `explain`, `facts`, `graph`, `learn`, `meaning`, `overview`, `prescribe`, `report`. Phase 2V (value tier, per [RFC 0004](./RFCS/0004-value-provenance-and-the-inward-boundary.md)) adds the inward half of the fact base — what a program reads, what it computes, what it chooses among, what it hands back — and the per-function summary that carries a value's origin across a call; `rustmanifest-resolve` is a second binary writing the resolution overlay every value-reading command accepts through `--resolution`.

**Landed since the semantic tier closed:** the resolved-name extractor as an opt-in SCIP overlay (`--scip`) reaching `facts`, `check` and the invariant surface, with edge provenance in traces; call resolution confined to cargo build-target boundaries; error-conversion edges synthesized from `From` implementations at `?` sites; macro expansion promoted to a standard analysis mode; multi-implementor dispatch over-approximated with the modality stated in the trace; ecosystem knowledge packs (`reqwest`, `tempfile`, `tokio`) plus standard-library knowledge, quoted in findings from the library's own documentation with the cost of a deviation counted rather than described; machine-applicable fixes applied by `check --fix`. Tooling hardening: `audit` subcommand as a tier-3 external-tool orchestrator, `versions` subcommand with `--fix` and `RM-VERSIONS-001` integration in `check`, Dependabot config, periodic CI workflow (udeps/hakari/mutants/semver-checks), and CI jobs for miri/sanitizers/cargo-careful/fuzz/cargo-geiger/versions. Graph cross-module fixtures were aligned with ra_ap 0.345 qualified-path resolution.

**Next:** Phase 3 (HTTP/OAuth — tier-3 clippy delegation already shipped as `check --delegate-clippy`), the value tier's own follow-ons (the four unresolved questions of [RFC 0004](./RFCS/0004-value-provenance-and-the-inward-boundary.md)), and the research plan's later phases (Ascent, tsetlin-rs ranking, MIR refiner). Planned maintenance: migrate error handling to `masterror` (P1, last), keep CI/tooling portable, and resolve pre-existing engine test failures observed under current toolchain.

**Source layout:** no source file exceeds 150 pure code lines (blank lines, `//`/`///`/`//!` and `#[cfg(test)]` blocks excluded); oversized modules are split into nested single-responsibility submodules, with the parent as `X.rs` beside the directory `X/` and never a `mod.rs`.

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

### Phase 1C — Tier-2 AST analyzer ✅ done (local)

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

### Phase 1D — Config, explain, cache ✅ done (local)

- `rustmanifest-config`: parse `rustmanifest.toml` (profile selection, per-rule severity overrides, include/exclude globs).
- Built-in profiles: `strict`, `default`, `minimal`, `ci`.
- `rustmanifest explain <RULE-ID>` — prints the rule rationale and methodology section to stdout.
- Inline unused-pragma finding (`RM-META-001`): pragma that references a rule that did not fire becomes a finding itself.
- Optional persistent cache (content-addressed by `(file_hash, rule_id_set)`). Behind `--cache <dir>` flag, default off.

### Phase 1E — Eval corpus and precision metrics ✅ done (local)

- `eval/` directory with 3–5 public Rust repositories as fixtures.
- Per-rule precision / recall measurement against curated labels.
- Baseline numbers committed; regression gate in CI for any drop > 2 percentage points.
- Performance budget enforcement: `p95 review_file < 100ms` for files < 1k LOC.

### Phase 1F — Reserved ✅ absorbed (cache landed in Phase 1D)

- Reserved for cache layer landing if not absorbed by Phase 1D.

### Phase 2 — MCP server (stdio) ✅ done (local; scripted stdio conformance tests shipped, external validator pending)

- `rustmanifest-mcp` built on the `rmcp` SDK.
- **Tools** (eight): `review_file`, `review_path`, `check_rule`, `lint_structure`, `explain_rule`, `project_overview`, `function_facts`, `learn_conventions`. The knowledge trio is the same surface the CLI prints, so an agent and a human read one understanding.
- **Resources**: `rustmanifest://methodology/{section}` — nine English methodology documents; every rule's rationale URI is gated against that list by test.
- **Roots**: workspace root taken from the process directory at startup; every client path is joined and canonicalized against it, and a result outside the root is refused.
- Scripted stdio conformance tests in CI; the external validator is still pending.
- All server-facing strings are English regardless of request language.
- **Not shipped**: prompts (`pr-review`, `pre-commit-check`, `security-audit`, `structural-audit`) — deferred, no prompt surface exists yet.

### Phase 2S — Semantic tier: fact base, behavior, ownership ✅ done (local)

Executes the plan in [RFC 0001](./RFCS/0001-semantic-tier-fact-base.md) and [RESEARCH-2026-07-behavioral-tier.md](./RESEARCH-2026-07-behavioral-tier.md) on the stable syntactic extractor.

- **Type graph** (`semantic/graph.rs`): structs, enums, traits, impls, methods, fields, interaction edges; absorbs derive and macro expansion output so generated types count as understood.
- **Module graph** (`semantic/module_graph.rs`): crate roots from Cargo target layout, `mod`/`use` resolution including Rust 2018 uniform paths, dependency cycle detection (`RM-STRUCT-011`), mermaid/dot diagrams via `rustmanifest graph`.
- **Behavioral facts** (`semantic/behavior.rs`): per-function reads, writes, constructs, error types, and effects (unsafe, filesystem, network, process, env, console); let-binding type inference for field owner resolution; exported via `rustmanifest facts`.
- **User-declared invariants** (`semantic/invariant.rs`): `[[invariants]]` in `rustmanifest.toml` — field writer allowlists (`write-only-from`) and effect bans (`deny-in`) — checked against the fact base, surfaced as `RM-INV-*` findings with full traces.
- **Ownership facts** (`semantic/behavior.rs`, `semantic/ownership.rs`): parameter modes (owned / borrowed / mutably borrowed), use shapes, clone and heap-allocation call sites; first reasoning rule `RM-MEM-001` — owned non-`Copy` parameter whose only observed use is field reads should be a borrow.
- **Call graph and transitive effects** (`semantic/calls.rs`): call candidates recorded per function (free, associated, and method calls with receiver-type inference), resolved conservatively — an edge only on a unique match, ambiguity counted as unresolved; per-function effect sets closed over the graph to a fixpoint, so a function calling a filesystem-touching function is known to touch the filesystem.
- **Loop ownership rules** (`semantic/loops.rs`, the §6b follow-ups): `RM-MEM-002` — a read-only clone of a loop-invariant place belongs before the loop; `RM-MEM-003` — a container allocated fresh every iteration that never escapes the iteration belongs outside the loop, cleared per pass.
- **Feature binarizer** (`semantic/features.rs`, `rustmanifest facts --features`): fixed-order named binary features per function — boolean predicates plus thermometer-encoded counts — the substrate for the interpretable learning layer and role classification (research doc §5).
- **Role heuristics** (`semantic/role.rs`): deterministic coarse role per function (io / mutator / constructor / accessor / pure / unknown), priority-ordered, surfaced in `rustmanifest facts` — the bootstrap labeling for the learning layer.
- **Interpretable learning layer** (`rustmanifest-learn`, `rustmanifest learn`): one deterministic Tsetlin machine per role over the binarized features, dumped as an auditable clause list (research doc §5, admitted by the §8 gate result: +17pp macro-F1 over logistic regression). Clauses describe the codebase; they are never a source of findings.
- Precision ceiling documented: move/borrow flow approximated by use-shape at the syntactic tier; real dataflow lands with the MIR refiner (research plan phase 6).

### Phase 2M — Meaning tier: framework models, external surface, subsystem capabilities ✅ done (local)

Executes [RFC 0002](./RFCS/0002-meaning-tier-framework-models.md). The semantic tier
answers structure and behavior; this phase answers what the program exposes and what
each subsystem is for, deterministically and with evidence per row.

- ✅ **Framework models as data**: the knowledge-pack shape extended with a crate's
  registration vocabulary — router constructors, route registrations, HTTP method
  wrappers, composition methods (`nest` with prefix, `merge` without), extractor
  types, listener constructors. A construct absent from the model produces no row.
- ✅ **Route recovery** (`semantic/surface/`): method, path, handler and byte range
  per registration; composition resolved through the existing call graph; a
  composition argument that is not a resolvable provider is reported as unresolved
  rather than silently dropped; runtime-assembled paths counted as dynamic; a
  conditionally registered route is recovered and marked, not lost. The table also
  counts what it could not read — unparsed sources, opaque handlers, framework
  vocabulary on a receiver never proven to hold a router — so silence is measured.
- ✅ **Handler contract**: request body, path and query parameters, shared state and
  application-defined extractors read from the handler signature; response payload
  from the return type.
- ✅ **Observable output** ([RFC 0003](./RFCS/0003-observable-output-fact.md)): a
  fact channel recording every write site with its stream, payload, guard stack and
  order (`semantic/behavior/output.rs`), entry-point identification
  (`semantic/entry.rs`), and a reachability grade, exported through
  `rustmanifest facts`. The coarse effect category is left untouched by decision;
  retiring it is separate work.
- ✅ **Meaning claims** (`semantic/meaning/`, `rustmanifest meaning`): one claim per
  write site — the stream, the text, and two certainties never merged into a score
  (reached from the entry, and runs on every path through its own function). A site
  the call graph never reaches keeps its claim and says so. The sentence degrades
  field by field: a weakened certainty names each construct that weakened it, a
  partially proven payload prints its proven text with the holes left as holes.
- ✅ **Canonical output forms** (`semantic/io_forms/`, rules `RM-IO-001`..`004`): the
  forms live as data — the properties the code must satisfy, the shape that
  satisfies them, what one deviation costs in counted bytes or calls, and the
  library guarantee the cost follows from. Checks contribute only facts; the message
  is assembled mechanically, so a finding can never claim more than the catalogue
  and the fact base license.
- ✅ **Prescription** (`rustmanifest prescribe`): the code the canonical form asks
  for, anchored to the line it replaces, with the counted gain stated beside it.
- ✅ **One-pass reports** (`rustmanifest overview`, `rustmanifest report`):
  everything the engine knows about a project, behaviour first, absent sections
  dropped rather than printed empty.
- ✅ **Subsystem capability** (`semantic/capability/`): what each module can do to the
  world outside the process — serves, fetches, stores, configures, persists, reports,
  spawns — read from the behavioral effect set and, where the category alone cannot
  tell a listening socket from a dialled one, from the written call that proved it.
  Each row names the function, that call, and its byte range, so a reader checks
  instead of believing. `capabilities` states it per module; `started` states which
  capabilities an entry point reaches through the call graph, keeping an unreached
  one as a row marked unreached rather than dropping it. A module whose functions
  touch nothing outside the process gets no row: silence means "no proof", never
  "no capability". `unsafe` proves none — it says how the code is checked, not what
  it can do. The proof channel itself is new fact-base surface
  (`FunctionFacts::effect_sites`), so an effect category is no longer an assertion
  without a citation.
- ✅ **Acceptance (route table)** (`rustmanifest-eval routes`): precision and recall of
  the recovered route table against the `#[utoipa::path]` endpoints projects declare
  independently of the router chain. Measured 2026-08-05 on the two API workspaces of
  the corpus: `ratma-studio-api` recall 100.0% (139/139) at precision 91.4%,
  `aura-api` recall 98.9% (92/93) at precision 83.6%. The measurement paid for
  itself immediately: it found three endpoints lost behind a `merge` fold in a
  method-router chain, which is now fixed, and the single remaining disagreement is
  a path the two sides spell differently rather than one the engine failed to see. The two rates are never merged
  into one figure, and the disagreeing endpoints are named rather than counted. Both
  sides are named for what they are: an *undeclared* row is recovered and not
  annotated, an *unrecovered* row is declared and not produced — which of the two
  tables is wrong is the question the measurement asks, not one it answers. The
  measurement reports and never gates: turning it into a threshold before anyone has
  read what it says would invite the numbers to be chosen rather than measured.
- ✅ **Acceptance (output)**: the smallest program in the language — a single
  unconditional greeting — yields exactly one claim naming the stream, the literal
  payload, and certainty, and every weakening of that program (a branch, a loop, a
  computed string, an early exit) degrades the claim field by field instead of
  collapsing it.

### Phase 2V — Value tier: the inward boundary, what is computed, where a value came from ✅ done (local)

Executes [RFC 0004](./RFCS/0004-value-provenance-and-the-inward-boundary.md), all
eight stages. The meaning tier states what a program writes out; this phase states
what it takes in, what it computes from it, what it chooses among, what it hands
back — and, through the per-function summary, where a written value came from when
the answer lies past a call.

- ✅ **The value term** (`semantic/behavior/value/`): where a value came from, as a
  written literal, one of the function's own input slots, an index into a recorded
  read, operation, choice or call, or a named refusal. Inside one body it is an
  index and never a subtree, so two consumers of one value name one record instead
  of carrying two copies. `FunctionFacts` gains `slots` — one entry per declared
  input, receiver first — so a call site's written arguments align with the
  callee's parameters even where the profiled parameter list drops an input it
  cannot read.
- ✅ **Handbacks and enriched call sites** (`semantic/behavior/handback.rs`,
  `collector/site.rs`): every site handing a value back to the caller, `?`
  included; every call carrying the arguments it hands over, the disposal written
  around it, its byte range and the conditions standing over it. Disposal is
  stamped post-hoc by an exact byte-range match — zero or several matches is
  `NotIdentified` rather than a guess.
- ✅ **The inward boundary** (`semantic/behavior/input.rs`, `collector/input_read.rs`,
  `collector/intake_chain.rs`): per read site — channel, selector, intake (text
  validated or raw bytes), terminator, shape, guard stack, conditions. Sources are
  recognized on the whole expanded path, so a configuration crate's own lookup is
  never credited to the process environment. An adapter table says which adapters
  keep positions; one that does not degrades the selector to unidentified while the
  channel stays proven.
- ✅ **Operations and choices** (`semantic/behavior/operation.rs`, `choice/`):
  language operators as a `const fn` over syn's operator enums; library methods
  that operate through a table stating what they hand back and whether they can
  fail, with the guarding recorded (unchecked, checked, saturating, wrapping,
  reported) and integer arithmetic never called panicking, because the build
  decides. A choice records its scrutinee and one alternative per written arm.
  Closure is proven three ways and the ground is stated: a catch-all, every variant
  of a standard type named — the weaker proof, resting on written constructor
  names — or unproven, which is where a `match` over a workspace enum with one arm
  per variant lands, since nothing in the fact base enumerates a type's variants.
- ✅ **Conversions as knowledge rows** (`semantic/knowledge/result.rs`,
  `std_knowledge/result.rs`): a row states what a method hands back and whether it
  can fail. Sixteen standard-library rows ship; the target is read only where the
  source writes it as the call's own type argument, never off the type a consumer
  declares. **Deviation from the RFC, deliberate**: the ecosystem packs
  (`reqwest`, `tempfile`, `tokio`) carry no method rows yet, so a conversion
  performed by one of them is not recorded as a conversion.
- ✅ **The condition stack** (`semantic/behavior/value/condition.rs`): which
  construct each enclosing frame is, one entry per guard frame, outermost first,
  with equal lengths asserted. This yields arm identity without touching `Guard`,
  its `const fn` export contract, or a single golden file.
- ✅ **The summary tier** (`semantic/summary/`): Tarjan condensation over the call
  graph emits callee-first in one pass — no fixpoint and no convergence argument —
  then substitutes a call's written arguments for the callee's slots. Every
  composition failure is named at the leaf rather than swallowed: recursion,
  unresolved or external callee, dispatch over several implementors, unwritten
  arguments, and a depth budget of 14 that emits its own refusal, so termination
  rests on the budget and not on an acyclicity argument.
- ✅ **The resolution overlay** (`rustmanifest-resolve`, `--resolution` on every
  value-reading command): which function a call reaches, what it hands back, what
  type a receiver has, and — since the fact base learned to read it — which type
  holds each field read and what type that field has, all from a loaded cargo
  workspace. The field rows are what a syntactic reading cannot reach at all: a
  value read out of a type another crate declares. Measured on the engine's own
  sources on 2026-08-06, after the reading rules had taken the syntactic tier as far
  as it goes: **92.3% without the overlay, 98.0% with it**. Three silences of the
  resolver closed most of the difference once they were lifted — the test
  configuration, under which a well-tested workspace hides most of its code; field
  reads written inside a macro argument, which are exactly the positions a format
  string prints; and the calls written beside them. A binary of its
  own so the rust-analyzer libraries — weekly `0.0.x`, no API stability — stay out
  of the tool a user runs on every save, and the engine takes the answers as plain
  data. A position the resolver cannot settle is absent from the overlay, so the
  engine reaches its own refusal instead of being handed a guess; a named overlay
  that cannot be read is an error and never a silent fallback.
- ✅ **Claims and the compound statement** (`semantic/meaning/intake.rs`,
  `claim.rs`, `narrative/`): intake, compute, choice and handback claims field for
  field like the write claim, every recorded site becoming one, unreached ones
  included. The statement decomposes instead of weakening — a clause resting on a
  refusal is named together with the question it failed on, while the licensed
  clauses still read as sentences. A grade below certain degrades the wording and
  names the weakener; it does not refuse.
- ✅ **Export** (`facts_export/`): reads, operations, choices, call sites and
  summaries ship on their own channels, each stamped with its recognizer-set
  version, so an empty channel reads as "the recognizer found none" and not as an
  absence of the thing.
- ✅ **Acceptance (the ladder)** (`crates/rustmanifest-engine/tests/ladder.rs`,
  seven tests): a calculator traces its printed result back to the argument it was
  read from and states its choice is closed and which arm leaves; a create-read-
  update-delete store traces the written value back to the collection it came from
  and claims nothing about the status beyond what it leaves; a tutorial program
  keeps the position it read through later adapters, states both sides of its
  fallback, and reads through the closure written beside a combinator. Every
  assertion failed before the work it guards.
- **What this tier refuses, by design**: a name bound or assigned more than once
  yields a named refusal, so an accumulator in a loop is stated as underived rather
  than guessed; there is no type inference, so a binding typed by a later use
  records no produced type; there is no rules tier over these facts — deriving
  comes first, cataloguing later.
- ✅ **The traced-position rate** (the honesty section of `rustmanifest report`): of
  the positions a program actually prints, how many have their value traced all the
  way through the composition. The strict number, and the one a generation gate
  would stand on — the whole-statement rate saturates once no clause has to speak a
  refusal, and cannot stand in for it. Measured on 2026-08-06: `masterror` 52.7%
  (1177 of 2233), `fork-hyper` 31.2% without the overlay and 32.8% with it, the
  ordinary task list 8 of 8. The ladder's own programs are fully traced: the
  greeting states its write with nothing left to derive, the calculator 41 of 41
  positions and its one written position traced to the command line, the
  create-read-update-delete store 39 of 39 and 1 of 1, the task list 141 of 141 and
  8 of 8. What stops the rest is one question and almost nothing
  else — which function a call reaches. It splits in two: a name the workspace does
  not declare at all now answers (the call leaves the program), and what remains is
  a name several workspace functions share, which only a receiver type settles. The
  overlay settles the receivers it can — 1609 calls of `fork-hyper`, 1860 of
  `masterror` — and the rest are generic-heavy sites it renders as unknown. That is
  where the remainder lives, and it is the resolver's coverage rather than the
  reading rules.
- ✅ **A call that leaves the program is an answer, not a gap**: what a function no
  workspace source declares hands back is decided outside the sources being read,
  which is as settled as the content of a field. Naming the callee states
  everything the program proves, and the refusal is kept for the case it was meant
  for — a name the workspace does declare that the binding rule cannot tie to one
  definition.
- ✅ **The whole-statement rate** (the honesty section of `rustmanifest report`):
  how many entry points get a statement that holds together against how many
  decompose because a clause rests on a refusal. It is the harder of the two
  numbers and the one a generation gate would stand on: a tier can answer most
  positions and still break every sentence. Measured on 2026-08-06 after a
  refusal at an unreached site stopped counting against a statement about the
  process: `masterror` 6 of 11, `fork-hyper` 16 of 18, and an ordinary
  command-line task list 1 of 1 with every one of its 141 value positions derived.
- ✅ **Acceptance (origins, off our own sources)**: the same measurement run on
  three independent workspaces of the eval corpus on 2026-08-06 — `masterror` 88.6%
  (20474 of 23100), `hydebar` 92.8% (49086 of 52870), `fork-hyper` 89.1% (16121 of
  18103) — against 88.3% on the engine's own sources. The reading rules are
  therefore not fitted to this repository's style: a rule that only worked on the
  code it was written against would show here as a rate that falls away from home.
  The figures report and never gate.
- ✅ **The reading rules the measurement paid for**, each one general unless stated:
  a path the standard library defines as a value — a unit variant, an associated
  constant of a primitive — is that value, from a table of library names; a name the
  workspace declares as a constant or a static is that constant, the body being
  asked first so a binding still shadows it; a field names its owner wherever the
  collector's own typing answers, instead of only on a parameter; what a loop hands
  its body is an element of what it iterates, recorded as the selection an index
  already is, and the same for the input of a closure written beside an adapter; a
  tuple pattern says which part each name holds, so taking a value apart keeps the
  origin; a conditional with an else chooses between two written alternatives and is
  recorded as that choice; a written structure is the value built at that position,
  and so is the value a standard macro builds; the propagating operator, an await
  and an unsafe block leave the same value standing; a block stands for the
  expression it ends with, and one that ends any other way is the unit value; a
  written tuple position, like an index, is a selection; a variant pattern names the
  payload it takes and a struct pattern the field it binds, the same rule reading a
  binding, a condition and an arm; a path of several segments names an item, which
  a binding never is; typing sees through a borrow as every other reading does; a
  macro of the library builds what the library says it builds, or ends the function
  and hands nothing back; what a resolver settled about a call and its receiver
  outranks the reading that infers from spelling; and two arms are two scopes, so a
  name bound in one never answers a reading in another — the one soundness bug the work found, which had
  been stating a value from the wrong branch.

### Phase 3 — Streamable HTTP, OAuth, clippy delegation ⏳ in progress (clippy delegation done locally; HTTP/OAuth pending)

- ⏳ HTTP transport with OAuth 2.1 + PKCE + RFC 8707 resource indicators.
- ⏳ Audience-scoped tokens, TLS 1.3 only, per-client rate limiting.
- ✅ Tier-3 delegation (`check --delegate-clippy`): `cargo clippy --message-format=json` parsed and surfaced under rustmanifest's reporting umbrella, each delegated finding carrying a rationale URI into the clippy lint index.
- ⏳ Optional `cargo audit` and `cargo geiger` integrations.

### Phase 4 — Sampling and elicitation ⏳ planned

- MCP `sampling` API for LLM-assisted classification of borderline findings.
- `elicitation` flow for interactive suggested-fix application.
- Per-prompt eval suite with golden labels; pass-rate gate before merge of any prompt change.

### Phase 5 — Integrations and editors ⏳ planned

- `rust-manifest/rustmanifest-action@v1` GitHub Action (composite): runs `rustmanifest check`, uploads SARIF, comments on PR.
- Pre-commit hook YAML snippet.
- `rustmanifest-lsp` — LSP server reusing the engine. Diagnostics, code actions, hover. The crate today is a skeleton: it declares the targeted protocol version and nothing else, so nothing the engine knows reaches an editor over LSP yet.
- Editor docs: Cursor, Zed, Helix, VS Code (LSP and MCP).

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
