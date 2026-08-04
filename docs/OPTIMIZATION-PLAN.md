# Optimization plan

Audit of the repository as of 2026-08-05, and the ordered work it calls for.
Every item states the measured or verified fact behind it, so the plan can be
checked against the code rather than believed.

**Goal this plan serves:** the engine has to be publishable and fast enough to
run on every save. Today the toolchain gate is broken, so nothing is verified,
and a single `check` parses each source file roughly thirty times, so nothing
is fast.

---

## Baseline measurements

Taken on this repository (466 Rust sources) with a `--release` binary:

| Command | Wall clock |
| --- | --- |
| `check crates` | 7.6 s |
| `report crates` | 15.0 s |
| `overview crates` | 3.8 s |
| `coverage crates` | 0.9 s |
| `facts crates` | 0.8 s |
| `cargo test --workspace --all-features` | 3.6 s (700 tests) |

State that is already sound and must stay sound: `cargo clippy --workspace
--all-targets --all-features` is clean under `deny` for `all`, `pedantic`,
`nursery` and `cargo` with no workspace-level allow overrides; no source file
exceeds 150 pure code lines (largest is 145); production code carries no
`TODO`, `todo!` or `unimplemented!`.

---

## 1. The toolchain gate is broken

`rust-toolchain.toml` was deleted by `4e4347a chore: up deps`, which in the
same commit raised `workspace.package.rust-version` from `1.95` to `1.97` and
`syn` from 2 to 3. The CI `msrv` job still asserts the deleted file's content:

```yaml
run: grep -q 'channel = "1.95.0"' rust-toolchain.toml || (echo "MSRV drifted" && exit 1)
```

Consequences, all verified:

| Fact | Evidence |
| --- | --- |
| The `msrv` job fails on the next push | the file does not exist |
| The `lint` and `test` jobs run on whatever `rustup` defaults to, while their steps claim the toolchain is "pinned by rust-toolchain.toml" | `.github/workflows/ci.yml` |
| The declared MSRV (1.97) and the version CI asserts (1.95) disagree | `Cargo.toml:22` vs `.github/workflows/ci.yml` |
| 225 commits on `main` have never been through CI; the last recorded run is from 2026-05-13 (PR #10) | `gh run list` |

**Work.** Restore `rust-toolchain.toml` pinned to the declared MSRV, and
replace the hardcoded grep with a check that reads `rust-version` out of
`Cargo.toml` and asserts the pin matches it. The gate then cannot drift again
without failing, which is the property the original grep was reaching for.

---

## 2. Every file is parsed by `syn` about thirty times per run

`Source` carries only a path and the file text (`source.rs`). Nothing holds a
parsed syntax tree, so each pass that needs one calls `syn::parse_file` itself.
There are 34 `syn::parse_file` call sites, about twenty of them on production
paths.

Per `check` over one file, with the bundled pack of 3 pattern, 8 AST and 14
semantic rules:

| Stage | Parses of that file |
| --- | --- |
| AST tier — `AstAnalyzer::analyze` parses per rule | 8 |
| Semantic tier — `SemanticAnalyzer::analyze` builds `TypeGraph::from_sources` per rule, and the graph parses every source | 14 |
| Semantic tier — each check's own `insights` parses the source again | up to 14 |

`TypeGraph::from_sources` has 31 call sites across the workspace; `overview`,
`coverage`, `graph` and `layout` each build their own.

**Work, in two independent steps.**

1. Give `Source` a lazily parsed tree behind a `OnceLock`, exposed as
   `Source::ast() -> Option<&syn::File>`, and route every production
   `parse_file` on an owned `Source` through it. The parse stays lazy, so a
   pattern-only run never pays for it, and a file that does not parse as Rust
   is still skipped exactly as before.
2. Build the workspace `TypeGraph` once per analysis and pass it by reference
   into `Check::insights`, instead of rebuilding it per rule. This is the
   dependency-inversion fix: the checks should receive the graph they depend
   on, not construct it.

Both are internal-signature changes; the public CLI and MCP surfaces do not
move.

---

## 3. The expensive tier runs sequentially

`check/pipeline.rs` fans tiers 1 and 2 out over the rayon pool through the
orchestrator, then runs the 14 workspace-wide semantic analyzers in a plain
`for` loop. The same shape is in the MCP composition (`mcp/src/analysis.rs`).

**Work.** Run the semantic analyzers through `par_iter` once they share one
prebuilt graph (item 2 must land first — a graph rebuilt per rule inside a
parallel loop would only multiply the waste).

---

## 4. Per-finding work that belongs per file

`SemanticAnalyzer::analyze` calls `pragma::collect(text)` inside the loop over
insights, so a file with *n* findings is scanned for pragmas *n* times.

**Work.** Collect pragmas once per source before the loop.

---

## 5. Sources are read from disk twice per `check`

`collect_findings` calls `read_sources` for the semantic tier and again for the
invariant pass, each time reading every file and allocating a fresh `String`.

**Work.** Read once, share the slice. `read_sources` also re-`stat`s each file
that the walker already visited; fold the size check into the single read.

---

## 6. The findings cache addresses the cheap half and is unsafe at scale

`FindingsCache` keys entries on a 64-bit FNV-1a hash of the file bytes plus a
rules fingerprint (`cache.rs`).

| Problem | Consequence |
| --- | --- |
| FNV-1a is not collision-resistant, and 64 bits over a large corpus is within birthday range | a collision silently serves another file's findings, rewritten onto this file's path — a wrong answer, not a miss |
| Entries are never evicted | the cache directory grows without bound, one entry per file content ever analyzed |
| The semantic tier is deliberately excluded from the cache | the cache covers the cheap per-file half and misses the half that dominates the runtime |

**Work.** Widen the key to 128 bits, bound the store, and add a workspace-level
cache entry for the semantic tier keyed on the whole analyzed file set. The
last part lands after items 2 and 3, since a per-rule graph rebuild would make
the cached and uncached paths disagree on cost but not on content.

---

## 7. Nothing guards the gains

`eval/baseline.json` and `eval/corpus.toml` exist and `rustmanifest-eval` runs
them, but no CI job does. `crates/rustmanifest-engine/benches/orchestrator.rs`
exists with no regression gate.

**Work.** A CI job that runs the eval corpus against the committed baseline and
fails on drift, and a benchmark job so items 2 to 6 cannot silently regress.

---

## 8. Dead weight and duplication

| Fact | Location |
| --- | --- |
| `rustmanifest-lsp` is an 18-line stub with three workspace dependencies, carried through every CI job and the release pipeline; its implementation is scheduled for Phase 5 | `crates/rustmanifest-lsp` |
| The `multiple_crate_versions` allow is duplicated verbatim, with the same five-line reason, in two crates | `lsp/src/lib.rs`, `learn/src/lib.rs` |
| 113 identical `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` blocks with one shared reason string sit above the test modules — roughly 550 lines of duplication | workspace-wide |
| `Makefile.toml` holds a single task and is written in Russian, against the repository's own rule that every artifact is English | `Makefile.toml` |

**Work.** Lift the crate-version allow to the workspace lint table, translate
`Makefile.toml`, and collapse the test-module allow blocks behind one macro
exported for test use. `rustmanifest-lsp` stays a member until Phase 5 unless
explicitly dropped — removing a published crate name is not a cleanup decision
to take silently.

---

## Order of work

Each step is committed on its own once `cargo clippy --workspace --all-targets
--all-features`, `cargo test --workspace --all-features` and a timed run of
`check` are green.

| # | Step | Depends on |
| --- | --- | --- |
| 1 | Toolchain pin restored, MSRV gate derived from the manifest | — |
| 2 | `Source::ast()` — parse each file once | 1 |
| 3 | One `TypeGraph` per analysis, injected into the checks | 2 |
| 4 | Semantic tier parallelized | 3 |
| 5 | Pragmas collected once per source | 3 |
| 6 | Sources read once per `check` | 3 |
| 7 | 128-bit cache key with a bounded store | 1 |
| 8 | Semantic-tier cache entry | 3, 7 |
| 9 | Eval-corpus and benchmark CI gates | 4, 5, 6 |
| 10 | Duplication and language cleanups | 1 |
