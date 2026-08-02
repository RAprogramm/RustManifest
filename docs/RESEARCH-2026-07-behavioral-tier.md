# Behavioral tier research — July 2026 state of the art

Synthesis of four research passes (tooling landscape, analysis infrastructure,
behavioral-fact best practices, Tsetlin machines) into an implementation plan
for the next tier of the engine: understanding what code *does*, not only what
it *is*, and enforcing user-declared domain invariants.

Verification legend: facts below were gathered from live sources in August
2026 unless marked as inference.

---

## 1. The niche is open

- No tool for Rust enforces field-level behavioral invariants ("field
  `Wallet.balance` may only be written from module `ledger`"). Verified by
  absence across every search: cargo-pup (DataDog, ~0.1.8) is the nearest
  competitor and stops at module/import/shape checks **and requires a pinned
  nightly** (`nightly-2026-01-22`); rust_arkitect and arch_test are
  `use`-graph only. A June 2026 users.rust-lang.org thread concluded no
  mature ArchUnit equivalent exists for Rust.
- CodeQL gained Rust GA (October 2025, extractor built on rust-analyzer) —
  it is the strongest reference for a fact schema and the main incumbent,
  but it is proprietary infrastructure with its own query language, not a
  cargo-native stable-toolchain tool.
- Error-type extraction ("which error types can this function return") has
  no dedicated tooling anywhere. First-mover territory.
- No published work applies Tsetlin machines to source-code classification
  (defect prediction, naming, code smells). Also first-mover territory.

## 2. Proven architecture to copy

Every serious fact engine (CodeQL, Glean, Kythe) converges on the same
pipeline, validated across the industry:

    dumb total extractor  →  typed fact base  →  declarative derivation

- Extraction stays mechanical: every field access becomes a primitive fact
  (`accesses(fn, field, mode, span)`), every call a fact, every signature a
  fact. No cleverness at extraction time.
- "Function F writes field X" is a **derived relation**, not a primitive:
  `writes(F, X) :- assigns(F, _, X). writes(F, X) :- calls(F, G), writes(G, X).`
- Derivation engine: **Ascent 0.8** (March 2026) — Datalog embedded in Rust
  proc macros, relations are plain Rust types, BYODS custom data structures,
  parallel evaluation, and `ascent-interpreter` for runtime-loaded rules
  (user-supplied invariants without recompiling). Actively maintained.
- Rule surface for users, two levels (the CodeQL-power / Semgrep-UX split):
  1. declarative TOML/RON in dependency-cruiser style:
     `deny { write = "Wallet.balance", except-from = "crate::ledger" }`
  2. raw Datalog escape hatch for power users.

## 3. Extraction layer choice

| Layer | Status July 2026 | Verdict |
|---|---|---|
| `syn` AST (current engine) | stable, no name resolution, no macro expansion | keep as tier-2; insufficient alone for reliable write-sets |
| rust-analyzer `ra_ap_hir` | 0.0.344 (July 27, 2026), weekly releases, **works on stable**, zero API stability promise | the practical sweet spot; same choice CodeQL and cargo-scan made; pin the version, budget churn tax |
| `rustc_public` (ex stable-MIR) | nightly-only, not on crates.io (MCP #949 open), SemVer crate is an aspiration | future refinement path, not a foundation today |
| Flowistry / rustc_plugin | alive (May 2026 releases), nightly-pinned | reference for MIR dataflow later |
| MIRAI | archived Aug 2024, fork stale since Mar 2025 | dead, ignore |
| contracts (`#[requires]/#[ensures]`) | nightly experiment, MCP #759, no stabilization | watch; align invariant syntax so migration is possible |

Decision: keep the fact **schema IR-agnostic**. Start extraction from what we
have (syn + our expansion absorption — already better than plain syn), add a
`ra_ap_hir`-backed extractor as an accuracy upgrade for name/type resolution,
leave a slot for a `rustc_public` refiner when it ships on crates.io.

## 4. Behavioral fact taxonomy

Primitive facts per function/method (all with spans):

- `reads(fn, type.field)` / `writes(fn, type.field)` — receiver-aware;
  `&mut self` methods without a summary are conservatively "may write Self".
- `constructs(fn, type)`
- `calls(fn, callee)` — already have interaction edges; formalize.
- `returns_error(fn, error_type)` — resolve `Result<_, E>` in signatures
  plus `From` conversions at `?` sites. Nobody does this; we should.
- `effect(fn, category)` — adopt the cargo-scan taxonomy (arXiv 2602.06466,
  Feb 2026): unsafe effects (raw deref, FFI, static mut), system effects
  (std fs/io/net/process/env sink patterns; same mechanism pointed at
  sqlx/diesel/redis for DB effects), higher-order effects (closures and fn
  pointers = opacity markers).
- Interior mutability: classify `.borrow_mut()/.lock()/.set()/.store()` on a
  field as writes to that field; detect `UnsafeCell`-carrying types by known
  std wrappers; document unsafe/FFI/trait-object opacity honestly. Whole
  soundness here is a research problem — we aim for "sound modulo a
  documented escape-hatch list", same as cargo-scan.

## 5. Interpretable learning layer: tsetlin-rs

Own crate, verified state v0.3.4 (Jan 2026 feature work, active deps):

- Deterministic: explicit `seed` on every training entry point, Xoshiro256++;
  inference is pure boolean clause evaluation — same input, same output,
  same explanation. Sequential training mode for full reproducibility.
- Interpretability is first-class: `Rule { included, negated, polarity }`
  extracted from clauses, `explain_string(x, feature_names)` renders
  "predicted X because name_len>=20=1 AND has_doc=0". The whole model dumps
  as a reviewable rule list.
- Missing piece (ours to build): feature binarization. Boolean predicates
  (`is_pub`, `returns_result`, `has_doc`, `writes_foreign_field`, ...),
  thermometer encoding for counts (arity, fields, LOC thresholds), one-hot
  for categories. The fact base from §4 is exactly the feature source.
- Field-tested guidance (2026 literature): 100–500 clauses for structured
  problems, literal-budget for short readable clauses, sparse TM for large
  token vocabularies, small-data regimes are TM's home turf — fits per-repo
  convention learning where examples number in the hundreds.
- Roles in the engine, all decision-transparent (no black box anywhere):
  1. rank findings by learned repo-specific acceptance (train on
     accepted/suppressed findings via `partial_fit` — online adaptation);
  2. learn project naming/structure conventions as explicit clauses the
     user can audit;
  3. classify entities (handler/repository/entity/service) from behavioral
     feature vectors to power layer rules without manual annotation.
- Novelty: no published TM-on-source-code work exists (verified). This
  combination — deterministic fact engine + clause-transparent learner —
  is unoccupied ground.

## 6. Verification adjacency (not our lane, but alignment)

- Kani: healthy, AWS-backed, production CI use (Firecracker, std-library
  campaign, 16k+ harnesses). Complementary: proves bounded properties in
  user-written harnesses; does not extract facts or enforce architecture.
- Creusot: active (POPL 2026 tutorial), deductive, Why3; for small verified
  cores only. Prusti: momentum lost.
- Contracts experiment annotated ~200 std functions (June 2026 paper).
  Takeaway: express our invariants in a syntax morally compatible with
  `requires/ensures` so a future bridge is cheap.

## 6b. Ownership and memory understanding (added after review)

The plan above covered *what code does* but not *how it holds memory* —
ownership, borrowing, and optimization advice. That is a required part of
the engine's understanding, phased the same way:

- **Ownership facts per function** (syntactic tier, works today):
  parameters classified owned / borrowed / mutably borrowed with their
  types; `.clone()` / `.to_owned()` / `.to_string()` call sites; heap
  allocation sites (`Box::new`, `Vec::new`, `vec![]`, `String::from`);
  how each owned parameter is actually used (only field reads vs moved
  on / mutated).
- **Reasoning rules over those facts** (deterministic, trace-carrying):
  an owned non-`Copy` parameter whose only observed use is field reads
  should be a borrow (`Copy`-ness known from the type graph's derive
  knowledge, including absorbed expansions); clone-inside-loop; repeated
  allocation where reuse is evident. Where clippy already owns a lint
  (`redundant_clone`, `needless_pass_by_value` on MIR), delegate rather
  than duplicate — our value is workspace context plus the derivation
  trace.
- **Precision ceiling** at the syntactic tier is documented honestly:
  move/borrow flow is approximated by use-shape, not borrow-checked; the
  MIR refiner (phase 6 below) is where real dataflow lands later.

## 7. Phased plan

1. **Facts tier in the engine** (stable toolchain, current extractor):
   read/write/construct/error/effect facts from syn-level bodies with the
   documented conservative rules; facts exposed via a `facts` subcommand
   (JSON) so everything is inspectable, coverage counters extended.
2. **Invariant rules**: TOML rule surface (`deny-write`, `only-from`,
   `layer`, `no-effect`) compiled to checks over the fact base; findings
   carry full derivation traces like every other tier. This is the
   "Wallet.balance only from ledger" milestone — the thing no Rust tool has.
3. **Ascent integration**: move derivation to Datalog rules once fact
   volume/rule complexity justifies it; evaluate `ascent-interpreter` for
   user-loaded rules.
4. **ra_ap_hir extractor**: name/type-resolved facts (pinned version),
   raising write-set precision from syntactic to semantic.
5. **tsetlin-rs layer**: binarizer over the fact base, seeded training,
   clause dump as auditable convention rules; ranking of findings first
   (lowest risk, immediate value).
6. **rustc_public refiner**: when it lands on crates.io.

Each phase lands behind the existing quality gate (clippy pedantic+nursery
deny, full tests, eval corpus regression check) with local commits only.

## 8. Admission-gate experiment result (2026-08-01)

The §5 gate — a Tsetlin machine must beat logistic regression on identical
features by >= 5pp F1 before any integration — was run as an out-of-repo
prototype the day the binarizer landed.

- **Setup**: 8327 functions from the four eval-corpus repos, the 37-feature
  binarized vectors from `rustmanifest facts --features`, labels from the
  engine's deterministic `role()` heuristic (io / mutator / constructor /
  accessor / pure / unknown), deterministic 80/20 split. `tsetlin-rs`
  v0.3.4 `MultiClass` (200 clauses, threshold 15, 30 epochs, fixed seed)
  against a one-vs-rest logistic regression trained on identical features.
- **Result**: Tsetlin accuracy 0.9988, macro-F1 0.9955; logistic regression
  accuracy 0.9640, macro-F1 0.8224. Margin **+17.3pp macro-F1 — gate
  passed**. The linear model loses exactly where roles are conjunctive and
  priority-ordered; clauses express that natively.
- **Interpretability confirmed**: the extracted positive clauses for `io`
  are literally the ground-truth rule, one effect per clause
  (`IF effect-console`, `IF effect-fs`, `IF effect-net`,
  `IF effect-process`) — the model is a readable rule list, not a black
  box.
- **Honest caveat**: labels are heuristic-derived from the same fact base,
  so this measures expressivity and clause recoverability, not discovery.
  The discovery-grade rerun happens when real acceptance labels
  (accepted vs suppressed findings) accumulate from actual use; until
  then the gate result licenses building the integration surface, not
  shipping learned rules as defaults.

## 9. Ground-truth experiment: syntactic resolver vs SCIP (2026-08-02)

Before committing to the §7 phase-4 extractor, the syntactic call
resolver was measured against a name-resolved ground truth instead of
guessed at.

- **Setup**: `rust-analyzer scip` (1.95/1.96 toolchain components, runs
  on stable) over the six eval-corpus repos; call edges reconstructed
  from occurrence ranges nested in definition `enclosing_range`s; the
  engine's per-function call sets exported by `rustmanifest facts` and
  joined on `(file, qualified name)`. In-workspace edges only; callers
  compared only where both sides analyzed the function.
- **Result** (precision = our resolved edge confirmed by SCIP, strict
  `Type::method` match; recall = SCIP workspace edge we resolved):

  | repo | callers matched | precision | recall |
  |---|---|---|---|
  | masterror | 79.5% | 97.4% | 81.3% |
  | fork-hyper (all features) | 83.2% | 92.7% | 79.6% |
  | hydebar | 91.4% | 95.5% | 72.8% |
  | entity-derive | 75.4% | 99.6% | 79.8% |
  | aura-api | 79.0% | 100.0% | 68.3% |
  | ratma-studio-api | 78.1% | 99.8% | 56.4% |

  Aggregate: **precision 96.9%** (11085/11435), **recall 74.3%**
  (13811/18590).
- **Failure taxonomy**. False resolutions concentrate in one class:
  name collision with an external type (39× `syn::Error::new` bound to
  the workspace `Error::new` in masterror-derive; `Bytes::from`,
  `h2::client::Builder` methods in hyper). Recall losses split into
  calls never seen as candidates — macro-generated bodies
  (`pin_project`'s `project`, derived impls, `thiserror`-generated
  `From::from` at `?` sites) — and candidates seen but not uniquely
  bound: trait-impl dispatch (`poll_*` families), `Default::default`,
  `.clone()` on derived impls.
- **Method caveats**: rust-analyzer indexes only cfg-active code — the
  first fork-hyper run with default features produced a blind ground
  truth (caller match 31.5%, apparent precision 78.0%) and was
  discarded; `cargo.features = "all"` restored validity. SCIP also
  counts operator desugarings (`?` as `Result::branch`, `.await` as
  `Future::poll`, arithmetic as trait calls) that the engine
  deliberately does not model as calls; these were excluded from edge
  sets by construction, not by filtering.
- **Verdict**: the conservative resolver is precision-grade (96.9%
  aggregate, four repos at or above 97%) — resolution-by-unique-match
  holds. The missing quarter of edges is structural: name resolution
  through macro expansion and trait dispatch, exactly what the
  syntactic tier cannot see. The phase-4 upgrade is therefore justified
  and its shape is settled by this experiment: consume the SCIP index
  as an optional accuracy layer (stable toolchain, 15–40 s per medium
  workspace, format stable) rather than linking `ra_ap_hir` — the
  PLAN.md §3.2 argument, now with numbers. Indexing cost keeps it
  opt-in; the syntactic resolver stays the fast default.
- **Priority signal for knowledge packs**: the same edges rank external
  callees by volume. After the std family, the top uncovered crates
  across the corpus are tokio (1877 calls), http (1633), syn (849),
  axum (825), serde_json (688). thiserror does not appear in call
  counts at all — its weight sits in derive-generated `From` impls,
  i.e. expansion knowledge, not method knowledge. The tokio pack
  shipped same day (fork-hyper: functions with a known network effect
  37 → 92); http and serde_json are next.

## 10. Deriving pack knowledge from dependency sources (2026-08-02)

Hand-written packs do not scale to the ecosystem. The alternative was
tested the day it was proposed: dependency sources already sit unpacked
in the cargo registry cache (`cargo metadata` maps every resolved
version to its path, `cargo fetch` fills gaps without compiling), and
the engine's own extractor runs on them unchanged — a dependency is
just another workspace. Effects should then derive from the std axioms
(`fs::`/`net::`/`process::`/`env::` path sinks) closed over the
dependency's internal call graph, and the three hand packs become the
gate: derivation is admitted when it reproduces them.

- **Setup**: `rustmanifest facts` over tempfile 3.27.0, reqwest 0.13.4,
  and tokio 1.53.1 (the latter jointly with mio and socket2 so closure
  can cross into the crates where sockets actually live). 33 hand-pack
  effect rows checked against derived per-function effect sets,
  cfg-duplicate definitions merged by union.
- **Result**: **8/33 rows reproduced, 0 contradictions, 21 with no
  derived effect, 4 with no matching function.** Derivation never
  produced a wrong effect — it is incomplete, not unsound. Joint
  analysis matters: `TcpStream::connect` gains `net` only when socket2
  is in the analysis set; `Response::{bytes,json,text}` reproduce
  inside reqwest alone but `RequestBuilder::send` cannot — its network
  bottom is in hyper, outside the analyzed set. Scale is a non-issue:
  tokio+mio+socket2 is 4564 functions, seconds per crate set.
- **Failure taxonomy** (each miss traced to its mechanism):
  1. *Import-blind effect classification* — `TempDir::close` calls
     `remove_dir_all` bare, imported via `use std::fs::remove_dir_all`;
     the path sink sees no `fs::` segment and stays silent. The module
     graph already resolves use targets; effect classification just
     never consults it. Cheapest fix, benefits workspace analysis too.
  2. *Trait-default attribution loss* — `read_exact`, `write_all` exist
     in the derived set as free functions: trait default methods lose
     their owner, so a row keyed `TcpStream::write_all` can never
     match. Extractor naming gap.
  3. *Receiver-typing gaps* — `NamedTempFile::keep` delegates to
     `self.path.keep()` and the field's type never binds
     (`?::keep`), so closure has no edge to follow.
  4. *Missing axiom floor* — tokio/mio socket reads bottom out in libc
     syscalls (`unsafe` FFI, no `std::net` path) and in method calls on
     std types (`Read::read` on an inner `std::net::TcpStream`), which
     path patterns cannot see. The axiom layer needs std *method*
     knowledge and a libc/syscall table, or those chains stay dark.
- **Verdict**: the architecture is validated in direction — extraction
  over dependency sources is sound, cheap, and needs no new
  infrastructure — but the gate is failed at 24% coverage, so hand
  packs stay as the knowledge source until causes 1, 2, and 4 are
  implemented. Order fixed by cost/benefit: use-resolved effect
  classification first (pure engine improvement, helps every tier),
  then trait-default attribution, then the std-method/libc axiom
  floor. Derived tables are additive only — a pack row always wins
  over silence, derivation fills where no pack exists.
- **Repair 1 shipped same day** (`use`-resolved effect classification):
  the file's imports become a name → full-path table (glob imports
  skipped, conflicting imports dropped, file-level granularity as a
  documented approximation), and the sink classifier expands a path's
  first segment through it before matching — a bare `remove_dir_all`
  imported from `std::fs` now classifies, including single-segment
  calls the classifier previously never saw. Gate reruns at **11/33,
  still 0 contradictions** (`TempDir::close`, `UdpSocket::connect`,
  `UdpSocket::send_to` flipped — the rows predicted by the taxonomy).
  Workspace side: fork-hyper functions with a known effect 263 → 295
  with the eval baseline byte-stable, so the gain is pure fact
  coverage, not finding drift. Remaining misses sit in causes 2–4 as
  predicted; none moved without their repair.
- **Repairs 2 and 4a shipped; gate did not move — negative result with
  a new cause.** Trait default methods are now extracted under their
  trait's name (unit-proven: a default body's effect lands on
  `Trait::method`), and a 47-row std method-effect table (File,
  TcpStream, UdpSocket, TcpListener, Command, Child, Stdin/out/err,
  OpenOptions, DirBuilder) sits under receiver-typed method calls as
  the axiom floor. The gate stayed at **11/33** because both repairs
  hit walls the taxonomy had not named:
  1. *Macro-wrapped declarations* — tokio and hyper wrap their ext
     traits in `cfg_*!` macro invocations, so no `trait` item exists
     for syn to see; the four `NO FUNCTION` rows stay. The repair is
     expansion, not extraction — same thread as thiserror derive
     knowledge.
  2. *Higher-order IO chains* — the axiom floor did light up inside
     the dependency graph (tokio+mio+socket2 functions with effects
     1407 → 1479), but tokio routes instance IO through closures
     (`do_io(|inner| inner.read(buf))`), and closure parameters are
     untyped, so the chain to the public method breaks. Documented
     higher-order opacity, now with a measured face.
  Eval baseline stayed byte-stable; fork-hyper gained trait-default
  functions (1225 → 1236) with effects nearly flat (295 → 296) — the
  corpus's own receivers are generic, exactly where the syntactic
  tier's ceiling sits. Conclusion: the remaining 22 rows are not
  reachable by cheaper syntactic repairs; they need expansion
  (macro-wrapped items) and either blanket-impl reasoning or the SCIP
  accuracy layer resolving ext-trait call sites directly. The
  derivation program pauses here by design — the next leverage is the
  phase-4 SCIP layer, which serves both this gate and the resolver's
  missing quarter from §9.

## 11. Phase-4 SCIP accuracy layer shipped (2026-08-02)

The extractor decision from §9 is implemented: the engine consumes a
`rust-analyzer scip` index as an opt-in overlay on the call graph.

- **Contract**: the layer is purely additive — overlay callee names
  resolve through the same unique-match tables as syntactic
  candidates, an edge the candidates already produced is not
  recounted, and a separate counter states exactly what the index
  added, so every edge's provenance stays distinguishable. The engine
  never runs the indexer itself; the index file is an input
  (`--scip` on `facts` and `coverage`), keeping the fast default
  untouched and the accuracy upgrade a deliberate choice.
- **Dependency decision**: the official protobuf bindings pull a
  second major version of an error-handling crate into the tree,
  tripping both the duplicate-version ban and the cargo lint wall.
  Instead of weakening either gate, the layer reads the wire format
  directly — a ~200-line decoder for the four fields the engine
  consumes (documents, paths, occurrence ranges/symbols/roles),
  skipping unknown fields by wire type. Field numbers were validated
  empirically against a live rust-analyzer index, not assumed.
- **Measured lift**: on the engine's own workspace the overlay adds
  290 edges on top of 1279 syntactic ones (+22.7% workspace edges);
  on masterror it adds 718 on top of 2479 (+29.0%). Transitive
  effects close over the enriched graph, so the lift propagates to
  effect knowledge wherever the index is supplied. Eval baseline
  byte-stable (layer off by default), REUSE and the full lint wall
  green.
- **Inherited limits, stated at the API**: the index reflects one cfg
  universe (the §9 all-features lesson is now a doc requirement on
  the flag), and operator desugarings are excluded by the same
  function-symbol filter that admits real calls. Attribution uses
  smallest-enclosing-definition ranges; nested items inside bodies
  attribute to the innermost function, same as the ground-truth
  harness measured at 96.9% agreement.

### First application runs (same day)

- **Where the lift comes from** (masterror, per caller-callee pair):
  of the overlay-addable pairs, 47% were calls the syntactic
  extractor never saw as candidates — macro-generated bodies, the §10
  cause confirmed from the consuming side; 30% had the method name
  under a different or uncertain owner; 22% sat in the
  unknown-receiver bucket a typed receiver would have settled. The
  index recovers exactly the classes the taxonomy predicted, in that
  order.
- **Against the §10 gate: unmoved at 11/33 — and the wall now has a
  name.** With the tokio index overlaying the tokio+mio+socket2
  derivation run, effect-carrying functions rose 1479 → 1495 but no
  gate row flipped, for two mechanically verified reasons. First, the
  four `NO FUNCTION` rows are trait defaults syn never sees; an
  overlay can only connect functions that exist as facts on both
  ends, and no index can conjure the missing endpoint. Second, the
  socket rows resolve in SCIP to methods that exist in facts under
  *colliding short names* — tokio and mio each define the same
  `Type::method` — and the overlay inherits the unique-match guard by
  design, so it refuses the ambiguous join rather than guess. The
  binding constraint is now fact-name granularity: facts key on short
  qualified names, SCIP symbols are globally unique, and the join
  discards that uniqueness. Module-qualified fact keys are the
  unlock, and they sharpen every resolver bucket, not only the
  overlay.
- **Granularity repair shipped same day.** The join now settles
  colliding short names through the symbol's full identity: the crate
  from the symbol header against the crate the fact's file belongs to
  (registry version suffixes and dash/underscore spelling tolerated),
  and the descriptor path against the fact's module path from the
  module graph, suffix-aligned. The overlay hands the call graph
  bound fact indices instead of names; a collision the identity
  cannot settle is dropped, never guessed. Measured: the overlay's
  lift grew from +290 to **+401** edges on the engine's workspace and
  from +718 to **+777** on masterror — the collision bucket was real
  and is now largely connected. The §10 gate still reads 11/33 with
  effect-carrying functions flat at 1495, and the residual cause is
  now isolated to one mechanism: tokio and mio route the actual IO
  sink through a closure at *every* level of the chain
  (`do_io(|inner| inner.read(buf))`), the closure parameter is
  untyped, so the effect never enters any fact — and edges, however
  perfect, cannot carry an effect that no endpoint holds. The next
  unlock for the gate is closure-parameter receiver typing, a
  behavior-extractor repair, not a resolution one.
- **Closure typing shipped same day — the gate's largest single jump:
  11/33 → 18/33, still 0 contradictions.** Single-generic wrapper
  fields now record their wrapped inner type, and a single-parameter
  closure passed to a method on such a field binds its parameter to
  that inner — the `do_io(|inner| inner.read(buf))` shape. The same
  class of approximation as the long-standing iterator-adapter
  element binding, documented as such; a two-generic wrapper stays
  unbound (the inner would be a guess). With the std method axioms
  underneath, the effect now originates inside mio's closures and
  flows up: TCP flush/peek/read/write and the whole UDP family
  flipped in one move (effect-carrying functions 1495 → 1515). The
  residual 15 rows decompose cleanly: four macro-wrapped trait
  defaults (expansion thread), five tokio wrappers of other shapes
  (readiness futures, `try_io`), the rest imp-module chains and the
  reqwest-hyper crate boundary. Eval baseline byte-stable throughout.
- **Two boundary probes closed the day.** The joint
  reqwest+hyper+hyper-util+tokio+mio+socket2 run (7140 functions,
  seconds) did not light `RequestBuilder::send`, and the break is now
  a single call: `Client::execute_request` reaches its transport as
  `?::call` — a tower `Service` invoked through a generic receiver.
  Service-trait dispatch joins the wall list alongside macro
  expansion; no amount of joint analysis crosses it syntactically.
  The expansion probe went the other way: tokio expanded via nightly
  `rustc -Zunpretty=expanded` on a scratch copy (3.8 MB stream, one
  cfg universe) and fed to the extractor restored every missing
  endpoint — all four `NO FUNCTION` rows now exist as
  `AsyncReadExt`/`AsyncWriteExt` facts, `read_exact` already carrying
  a transitive `unsafe`. Their network effect still waits on two
  known steps: generic `poll` bodies hold the sink, and the gate keys
  by concrete type, so ext-trait facts need blanket-impl reasoning
  to reach `TcpStream::write_all`. The derivation program's remaining
  walls are therefore three, all named: blanket-impl bridging,
  generic-body sinks, and service-trait dispatch.
- **Two dispatch mechanisms shipped against the first two walls; gate
  unmoved at 18/33, and the wall sharpened.** Generic function
  parameters now type as their first trait bound (inline or `where`) —
  the rule trait objects always followed — so a call on a generic
  parameter dispatches over the bound's implementors. Extension-trait
  defaults joined resolution from both sides: an unmatched
  `Type::method` reaches the unique `trait Ext: Base` default whose
  supertrait the type implements, and dispatch on an ext-trait
  receiver falls through the supertrait chain to real implementors.
  Both mechanisms are unit-proven end to end — the test's network
  effect crosses an ext default into a concrete impl and back to the
  caller. The gate rows still did not move because tokio's remaining
  chains break one level deeper than either mechanism reaches: the
  future types carry their bounds on the *impl block*, not the method
  signature (`impl<W: AsyncWrite> Future for WriteAll<'_, W>`), and
  the receiver arrives through a `Pin` projection of a
  generic-typed field. Impl-level bound propagation into field typing
  is the sharpened next step; eval stayed byte-stable and 347 unit
  tests are green throughout.
- **The sharpened step shipped — and the chain stopped one link from
  the end.** Three typing mechanisms landed together: impl-block
  generic bounds now map into field and binding typing inside the
  block's methods (`impl<W: AsyncWrite> ... { self.writer }` types the
  field as the bound); `Pin::new(&mut *value)` is transparent for
  receiver typing; and awaiting an expression whose future type is
  known records a `poll` candidate on that future, guarded to types
  that actually declare one. Unit-proven end to end (349 green), +39
  effect-carrying functions in the expanded tokio universe
  (1438 → 1477), eval byte-stable. The write-family gate rows still
  read dark for exactly one remaining reason, now isolated to a
  single name: pin-project-lite expands every future's projection to
  a struct called `Projection`, so the unique-name field table drops
  all of them and `me.writer` unresolves. Projection transparency is
  macro-semantic knowledge — the same expansion-knowledge thread as
  thiserror derives — not another typing rule. Gate holds at 18/33
  with 0 contradictions.
- **Projection transparency shipped — the chain closed end to end.**
  The knowledge is two constants: the projection method names and the
  view type names the macro fixes for every expansion; a projection
  call on a typed receiver types as the receiver when the declared
  return either names a view or is invisible (the generated `fn
  project` lives inside a `const _` block the item walk does not
  enter, so an unknown return is itself the macro's signature). With
  that one link, the full derived chain carries effects with no hand
  pack in the loop: a consumer's `stream.write_all(b"x").await` walks
  ext-dispatch into the expanded default, the await edge into the
  future's `poll`, the projection into the generic field, the bound
  into `AsyncWrite::poll_write`, and dispatch into implementors —
  `ReadExact::poll` now carries transitive `net`, the consumer
  carries it too, and the expanded universe rose 1477 → 1494
  effect-carrying functions. The cost is the documented dispatch
  over-approximation: every implementor's effects join, so a `poll`
  can inherit `process` from a child-pipe implementor it never
  touches at runtime. Eval byte-stable, 350 unit tests green.
