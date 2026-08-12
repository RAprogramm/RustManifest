# Audit: work already done by existing tools

<p align="right"><strong>English</strong></p>

**Date:** 2026-08-12 · **Scope:** the whole workspace · **Verdict:** about 15,400 of 33,797 production lines are re-derivation and should become adapters over other tools' output.

---

## 0. Why this document exists

The engine derives facts about Rust code. Some of those facts are already
derived, better and for free, by programs that ship today: the compiler,
its lints, the language server, the documentation emitter, the MIR
tooling, the cargo ecosystem. Every line spent re-deriving one of them is
a line that must be maintained forever and will still be wrong at the
edges.

This audit measures how many such lines exist, names the tool that
replaces each cluster, and states the shortest path out. It is a ledger,
not a plan of record; section 6 is the only part that proposes work.

**The rule it establishes:** before writing a derivation, name the tool
that already produces the fact and read its output. A hand-written
derivation is admissible only after the roster in section 2 comes up
empty, and the module doc must then say which tools were ruled out and
why.

---

## 1. Measurement basis

All figures are **production code lines**: non-blank, non-comment lines
in `crates/*/src`, with every `#[cfg(test)]` body excluded. Test code and
integration-test directories are counted separately and never mixed into
a class total.

| Surface | Lines |
|---|---|
| Production code in `crates/*/src` | 33,797 |
| Test code inside `#[cfg(test)]` | 14,821 |
| Engine crate, production | 25,597 |
| Engine `semantic` subtree, production | 22,990 |

Three earlier numbers circulating in review notes — 52,951, 37,735,
33,551 — are `tokei` totals that count test bodies and doc comments as
code. They are not wrong, they answer a different question. Nothing below
uses them.

Two facts frame everything else:

- The engine walks the syntax tree by hand. `syn` is referenced in 141
  files under `crates/rustmanifest-engine/src`.
- The engine links no language server at all. The `ra_ap_*` crates are
  pinned in the root manifest and appear in exactly one crate,
  `rustmanifest-resolve`, which is 277 production lines.

---

## 2. The roster of existing fact sources

Surveyed August 2026. A claim of duplication below names one of these
and, where an API is asserted, the exact entry point.

**Compiler-grade IR and semantics.** `rustc_public` — a SemVer-compliant
public API over MIR, stabilization tracked as a 2026 project goal;
`stable-mir-json` — the same MIR serialized to a file so a consumer needs
no compiler linkage; Charon — MIR cleaned into ULLBC/LLBC, a control-flow
graph carrying contextual and semantic information, built explicitly as
substrate for other analyses; Flowistry — MIR information-flow and
dependency analysis, modular by design and never descending into a
callee; MIRAI — interprocedural abstract interpretation over MIR with
value flow, panic reachability and taint; the language server crates
`ra_ap_hir`, `ra_ap_ide_db`, `ra_ap_ide`, `ra_ap_ide_assists`; the SCIP
index format.

**Verification.** Miri for undefined behaviour by interpretation, Kani
for bounded model checking, Creusot and Prusti and Verus for deductive
contracts.

**API surface and structure.** rustdoc JSON, typed by `rustdoc-types`;
`cargo-semver-checks`, whose lints are declarative queries over that JSON
rather than hand-written passes; `cargo-public-api`; `cargo metadata` and
guppy; `cargo-modules`, including module-cycle detection.

**Layout and size.** `-Zprint-type-sizes` with `top-type-sizes` for the
size, alignment, field offsets and padding of every type in use;
`cargo-llvm-lines`; `cargo-bloat`.

**Lints and hygiene.** clippy; rustc's own lints; Dylint, which runs
project-specific lints as dynamic libraries on the compiler driver;
`cargo-audit`, `cargo-deny`, `cargo-geiger`, `cargo-udeps`,
`cargo-llvm-cov`.

---

## 3. The ledger

| Class | Lines | Share of 33,797 |
|---|---:|---:|
| **Delete** — the tool already reports it to the user; nothing to adapt | 1,753 | 5.2% |
| **Replace** — must go, needs a thin adapter reading a tool's output | 10,700 | 31.7% |
| **Thin layer** — substrate available from a tool, real derivation on top | 10,070 | 29.8% |
| **Keep** — no existing equivalent | 2,657 | 7.9% |
| **Unaudited** | 8,617 | 25.5% |

The thin-layer class is not a reprieve. Roughly 30% of it — about 3,000
lines — is the substrate half that disappears once the tool supplies it,
leaving the derivation above untouched. Adding that to Delete and Replace
gives **about 15,400 removable lines against roughly 3,000 lines of new
adapter code.**

The unaudited 8,617 lines are not innocent, only unmeasured: the
remaining semantic modules (1,459), the language-server surface (1,085),
the overview tier (498), the protocol server (429), fact export (288),
and assorted argument and error plumbing.

---

## 4. The largest single clusters

Every row is measured, and every tool named is the entry point that would
answer the same question.

| Cluster | Lines | Replaced by |
|---|---:|---|
| `semantic/surface/` framework fold | 996 | partly `Semantics::type_of_expr`; the route table itself stays |
| `rustmanifest-rules-core` rule dispatch | 895 | declarative queries over a fact base, as `cargo-semver-checks` does over rustdoc JSON |
| `semantic/extract*` extraction assists | 847 | `ra_ap_ide_assists::assists`, composed with `move_module_to_file` |
| `behavior/collector/` typing cluster | 655 | `Semantics::type_of_expr` |
| `behavior/collector/` flow cluster | 471 | MIR control-flow graph; every `break` is already lowered |
| `semantic/moved*` use-after-move | 457 | rustc borrow check, E0382 — the module's own message quotes the code |
| `semantic/coverage/` | 428 | nothing; it measures the shortfall of the hand-rolled substrate and reads full once facts come from a resolver |
| `semantic/graph/` type graph | 484 | `Type::autoderef`, `type_arguments`, `as_adt_with_args` |
| `semantic/calls/` context and tables | 446 | `Semantics::resolve_path` |
| `behavior/signature/` | 389 | `Function::assoc_fn_params`, `Param::ty`, `SelfParam::access` |
| `semantic/std_knowledge/` return tables | 376 | `Semantics::type_of_expr`; the tables are hand-typed rows |
| `semantic/module_graph/` | 361 | `Crate::root_module`, `Module::children`; `cargo-modules` for cycles |
| `behavior/tables/` | 353 | `Function::ret_type`, `Field::ty` |
| `semantic/dead.rs` | 130 | rustc `dead_code`, already delegated through the check pipeline |
| `semantic/layout.rs` | 106 | `-Zprint-type-sizes`; the table is stated for 64-bit targets only (`layout.rs:21-28`) and is silently wrong elsewhere |
| `rustmanifest-cli/src/manifests.rs` | 119 | `cargo metadata`; the current reader scans lines and matches `[package]` textually (`manifests.rs:60-75`) |
| `rustmanifest-report/src/sarif/` | 171 | `serde-sarif` |

Two of these deserve a note beyond the line count.

**The SARIF emitter is not merely duplicated, it is unusable by its
stated consumer.** `Region` serializes only `byteOffset` and `byteLength`
(`sarif/document.rs:111-119`, filled at `sarif/conversion.rs:51-58`).
Code-scanning ingestion keys on `startLine`, which is never emitted.

**The use-after-move analysis restates a compiler error on code that, by
construction, does not compile.** The module doc says so itself:
"the compiler rejects it as E0382" (`semantic/moved.rs:13`), and the
finding text repeats the code (`semantic/moved.rs:67`). The defence —
that it runs before a build — is real, and it costs 457 lines to say
early what the compiler says for free the moment the file is valid.

---

## 5. The single most expensive mistake

**Not linking the language server into the engine: on the order of 9,600
lines.**

`ra_ap_hir` is pinned in the root manifest and linked by one crate of 277
production lines. That crate already calls the right APIs — the type of
an expression at `resolve/src/workspace.rs:198` and `:202`, method
resolution at `:231` — but only for two syntax nodes: field expressions
and method calls (`workspace.rs:157,168`). Its output crosses into the
engine through a single reader that takes the settled return type and
ignores both the resolved receiver and the entire resolved-fields vector
(`semantic/settled.rs:44-50`).

Everything downstream then keys facts by the last path segment, because
no stable identity was available. That is the origin of the by-name
lookups, the last-segment collapse that cannot distinguish two same-named
error types from different crates, and the refusal categories. On top of
that sits a second layer whose only job is to count and name the
resulting blindness — the coverage tier, 428 lines, plus the enumerated
refusal reasons in the call and provenance tiers.

The compounding is the point: roughly 9,600 lines of re-derivation, plus
several hundred more spent confessing it, all downstream of one loop that
was never widened from method-call expressions to expressions.

---

## 6. The shortest path out

**Widen the producer, not the consumer.** The reader already exists and
already states the right principle in its own module doc: nothing in it
depends on where the answers came from. The work is confined to
`crates/rustmanifest-resolve`.

1. Walk every expression, not only field and method-call expressions, and
   record its type keyed by byte range. Removes the typing cluster, the
   signature tier, and the by-name candidate machinery.
2. Add path resolution and emit a resolved-path overlay. Removes the
   naming and resolution modules, the binding and scope halves of the
   summary tier, and the call context tables.
3. Emit the module tree from the crate root. Removes the module-graph
   tier and the index identity module.
4. Emit usages per definition. Removes the borrow-fix scan, the buffering
   scan, and the reference half of dead-code detection.
5. Emit function signatures and field types for the whole workspace.
   Removes the field, return, variant and constant tables.
6. Expose the assist entry point behind a subcommand. Removes the
   extraction and implementation assists; the unlinked-file case follows
   from the server's own diagnostic.

Cost: the resolver grows from 277 to roughly 900 lines, plus about 250
for an assists driver. Removes on the order of 9,600.

**Second lever, only after the first lands.** rustdoc JSON read through
`rustdoc-types`: roughly 1,900 lines out for a 450-line adapter, and
`semantic/oracle.rs` is already most of that adapter, written by hand
against untyped JSON values. Two limits verified during this audit and
worth recording: rustdoc JSON does not follow type aliases, and it strips
private fields. Anything needing either must come from the language
server instead.

**Third lever.** MIR, through `rustc_public` or `stable-mir-json`:
roughly 1,700 lines out for an 800-line adapter, and the real price is a
nightly driver in the build. Take it only when the first two have landed.

---

## 7. What stays ours

The 2,657 lines with no existing equivalent, and — this is the load-bearing
half — the derivations sitting on top of the thin-layer class. Named, with
what was ruled out:

- **Module-scoped authorization invariants** with per-hop certainty.
  Ruled out: clippy's disallowed-methods configuration (workspace-global,
  no module predicate, no transitivity), `cargo-deny` (crates, not
  modules), `cargo-modules` (cycles only), MIRAI (requires tag calls
  edited into the source), Creusot and Prusti and Verus (per-function
  contracts).
- **The framework route-table recovery.** Routes are values, not
  declarations; no API emitter sees them.
- **The provenance vocabulary and the value tier.** Ruled out: the
  rustdoc schema (declarations), serialized MIR (an IR dump), SCIP
  (occurrences), `cargo-semver-checks` (API surface).
- **The entry-relative claim ledger and prose assembled from a fact
  graph** with holes never guessed. Verification tools produce
  counterexamples, not statements; the documentation emitter transports
  author prose rather than deriving it.
- **Loop-invariant hoisting decisions.** No clippy lint hoists an
  invariant clone or a per-iteration allocation out of a loop;
  `redundant_clone` was probed on the fixture and stays silent.
- **Derivation cited to the standard library's own sources.** Corrected
  upward during this audit: a stock sysroot ships no MIR for the print
  path, and rustdoc JSON strips the private writer chain, so neither
  source can answer it.
- **The priced output forms** and the deviation costs attached to them.
- **The resolver crate itself** — the one correct adapter already in the
  tree, and the template for every adapter this document proposes.

---

## 8. How to re-derive these numbers

Production lines exclude every `#[cfg(test)]` body; a `tokei` run alone
will not reproduce them. Counting rule: non-blank lines that do not begin
with `//`, in files under `crates/*/src`, stopping the production count at
the first `#[cfg(test)]` attribute in each file and attributing the
remainder to tests.

Cross-checks used above, each runnable against a clean checkout:
`grep -rl 'ra_ap_' crates` returns one crate and one module;
`grep -rl 'syn::' crates/rustmanifest-engine/src` returns 141 files;
`grep -rn 'startLine' crates/rustmanifest-report/src` returns nothing.

---

## 9. Status of the first lever

The producer has been widened. The resolver now records, for every file of
the analyzed workspace, the type of every expression and the item every
written path reaches, each keyed by byte range. Measured on this workspace
in one run:

| Row kind | Positions answered |
|---|---:|
| Expression types (new) | 73,737 |
| Resolved paths (new) | 36,163 |
| Method calls (existing) | 14,606 |
| Field accesses (existing) | 6,979 |

The two new kinds answer five times as many positions as the two the
overlay carried before, and they answer the two questions no reading of
spelling can: what type this expression has, and which item this name
reaches — the latter with the declaring crate attached, so two namesakes
stop collapsing into one bucket.

The consumer has taken the first of them: the type of an expression is now
read from the overlay before any spelling is examined, and the
hand-written reading answers only where no resolver ran. The reading is
kept, not deleted, until a measured run shows what it still answers that
the overlay does not — deleting it on the strength of an argument rather
than a measurement would repeat the mistake this document is about.

Locals are deliberately still unanswered. A local's identity is the
binding it was declared by, not a path, and a name string would read as an
identity while telling two shadowed bindings apart no better than the text
does. That row kind is owed its own shape.

## 10. Opinion

The engine's premise is sound and the audit does not touch it: a fact
base about a program, with every claim carrying the site that proved it,
is not something any tool in section 2 produces. What went wrong is the
substrate. Roughly two lines in five under `crates/*/src` exist to
approximate answers that a library already pinned in this workspace gives
exactly, and a further tier exists to measure how badly the approximation
does. That tier reading below 100% was never a coverage problem; it was
the design telling us the answer, and it was read as a backlog.

The correction is one crate wide. Everything above the resolver was built
to be independent of where the answers came from, which means the
expensive part — the tiers worth keeping — survives the change intact.
