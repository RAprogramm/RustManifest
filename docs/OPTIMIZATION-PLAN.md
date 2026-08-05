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

## Where it stands

Measured the same way after items 1 to 5 landed:

| Command | Before | After | |
| --- | --- | --- | --- |
| `check crates` | 7.6 s | 0.71 s | 10.7x |
| `report crates` | 15.0 s | 2.85 s | 5.3x |
| `overview crates` | 3.8 s | 1.28 s | 3.0x |
| `meaning crates` | 2.3 s | 2.03 s | 1.1x |
| `coverage crates` | 0.9 s | 0.64 s | 1.4x |

`check` reports the same findings it did before, judged by rule, file and
message; only byte offsets move, and only in the files this work edited.

State that is already sound and must stay sound: `cargo clippy --workspace
--all-targets --all-features` is clean under `deny` for `all`, `pedantic`,
`nursery` and `cargo` with no workspace-level allow overrides; no source file
exceeds 150 pure code lines (largest is 145); production code carries no
`TODO`, `todo!` or `unimplemented!`.

---

## 1. The toolchain gate is broken — done

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

## 2. Every file is parsed by `syn` about thirty times per run — done

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

**What landed.** The tree went onto `Source` itself rather than a separate
wrapper: `Source` never needed to be `Sync`, so a `OnceCell` on it works and
no signature outside the engine had to move. `Derivations` holds the tree and
the pragmas beside it, keeping `Source` within the field limit its own rules
set. Above that, `Workspace` holds what a whole analysis derives — type
graph, module graph, behavioral facts, resolution context, call graph — each
lazily and once, and every reader takes it as an argument. That last part is
what the measurements above are mostly made of: eight of the fourteen checks
read behavioral facts, and one `report` used to extract them six times.

---

## 3. The expensive tier runs sequentially — and must stay that way

`check/pipeline.rs` fans tiers 1 and 2 out over the rayon pool through the
orchestrator, then runs the 14 workspace-wide semantic analyzers in a plain
`for` loop. The same shape is in the MCP composition (`mcp/src/analysis.rs`).

**Verdict: not doable, and no longer worth doing.** A `syn::File` is neither
`Send` nor `Sync` — `proc_macro2::Span` carries a marker that forbids it —
so a parsed tree, and anything derived while holding one, cannot cross a
thread boundary. Sharing one parse across rules therefore rules out
parallelising over rules; the two are alternatives, not additions. Sharing
won by a wide margin: with the parses and derivations shared, the whole
semantic tier over this repository takes 0.7 s, where fourteen rules each
re-deriving everything took 7.5 s. Parallelising the old shape over eight
cores could not have reached that.

The parallelism that remains is the one that pays: tiers 1 and 2 run across
files, each file owning its own source and its own tree, so nothing crosses.

---

## 4. Per-finding work that belongs per file — done

`SemanticAnalyzer::analyze` calls `pragma::collect(text)` inside the loop over
insights, so a file with *n* findings is scanned for pragmas *n* times.

**Work.** Collect pragmas once per source before the loop.

**What landed.** The pragmas moved onto the source beside the tree, so all
three tiers share one scan per file instead of one per rule per file.

---

## 5. Sources are read from disk twice per `check` — done

`collect_findings` calls `read_sources` for the semantic tier and again for the
invariant pass, each time reading every file and allocating a fresh `String`.

**Work.** Read once, share the slice.

**What landed.** One read, shared by both tiers, and skipped entirely when
neither tier is enabled. The `metadata` call before each read stays: it is
what enforces the per-file memory budget, and reading first to measure
afterwards would defeat the budget it exists for.

---

## 6. The findings cache addresses the cheap half and is unsafe at scale — done, except the last part

`FindingsCache` keys entries on a 64-bit FNV-1a hash of the file bytes plus a
rules fingerprint (`cache.rs`).

| Problem | Consequence |
| --- | --- |
| FNV-1a is not collision-resistant, and 64 bits over a large corpus is within birthday range | a collision silently serves another file's findings, rewritten onto this file's path — a wrong answer, not a miss |
| Entries are never evicted | the cache directory grows without bound, one entry per file content ever analyzed |
| The semantic tier is deliberately excluded from the cache | the cache covers the cheap per-file half and misses the half that dominates the runtime |

**Work.** Widen the key to 128 bits, bound the store, and add a workspace-level
cache entry for the semantic tier keyed on the whole analyzed file set.

**What landed.** The key is a 128-bit FNV-1a, and the entry now records the
byte lengths of the file and of the rule-set fingerprint it was built from.
A lookup whose lengths disagree is a miss, so an address collision costs a
re-analysis instead of answering about a file it never saw. The store prunes
its least recently modified entries down to a capacity when it is opened —
one directory read per run rather than one per file. A run with the cache
enabled produces byte-identical output to a run without it.

**The semantic-tier cache is dropped.** It was justified when that tier took
7.5 s; it takes 0.65 s now, and its key would be the whole analyzed file set,
so any edit to any file misses. In the loop the cache exists for — edit,
re-check — it would miss every time and add a write of the whole result set
for nothing. Reintroduce it only if a future tier makes the workspace pass
expensive again.

---

## 7. Nothing guards the gains — done, differently than planned

`eval/baseline.json` and `eval/corpus.toml` exist and `rustmanifest-eval` runs
them, but no CI job does. `crates/rustmanifest-engine/benches/orchestrator.rs`
exists with no regression gate.

**The eval corpus cannot be that gate.** Its six entries are paths to local
checkouts, and two of them are private repositories; CI has no way to
materialize them. The harness stays a gate the maintainer runs, and this is a
property of the corpus, not an omission to fix later.

**What landed instead**, built out of what CI can actually reach:

| Gate | What it catches |
| --- | --- |
| `cargo bench --workspace --no-run` in the test job | benchmarks silently rotting out of compilation |
| a `self-analysis` job running `check` over this workspace | the engine failing outright on a real workspace, on every push |
| the same job running `overview`, `coverage`, `facts`, `graph`, `learn`, `meaning` and `report` | a command that answers for the small fixtures but not for a real one |

Each analysis step carries a timeout far above the sub-second time it takes,
so an order-of-magnitude regression fails the build while ordinary runner
load does not.

---

## 8. Dead weight and duplication

| Fact | Location |
| --- | --- |
| `rustmanifest-lsp` is an 18-line stub with three workspace dependencies, carried through every CI job and the release pipeline; its implementation is scheduled for Phase 5 | `crates/rustmanifest-lsp` |
| The `multiple_crate_versions` allow is duplicated verbatim, with the same five-line reason, in two crates | `lsp/src/lib.rs`, `learn/src/lib.rs` |
| 113 identical `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` blocks with one shared reason string sit above the test modules — roughly 550 lines of duplication | workspace-wide |
| `Makefile.toml` holds a single task and is written in Russian, against the repository's own rule that every artifact is English | `Makefile.toml` |

**Work and verdicts.**

`Makefile.toml` is translated. The other three are deliberately left alone,
each for a reason worth recording:

| Item | Verdict |
| --- | --- |
| `multiple_crate_versions` in two crates | **keep.** Lifting it to `[workspace.lints.clippy]` would be a workspace-level allow override, which this repository does not permit. The duplication is two attributes; the alternative is a hole in the lint wall for every crate. |
| the 113 test-module allow blocks | **keep.** Collapsing them needs a macro wrapping each `mod tests` body, which costs every test module its plain-Rust shape and its IDE navigation. The repetition is the honest price of denying `unwrap_used` in production code while letting tests fail loudly. |
| `rustmanifest-lsp` | **keep.** Removing a crate is not a cleanup decision to take silently; it stays a member until Phase 5 or an explicit call to drop it. |

---

## Order of work

Each step is committed on its own once `cargo clippy --workspace --all-targets
--all-features`, `cargo test --workspace --all-features` and a timed run of
`check` are green.

| # | Step | State |
| --- | --- | --- |
| 1 | Toolchain pin restored, MSRV gate derived from the manifest | done |
| 2 | `Source::tree()` — parse each file once | done |
| 3 | One workspace derivation per analysis, injected into every reader | done |
| 4 | Semantic tier parallelized | dropped — see item 3 |
| 5 | Pragmas collected once per source | done |
| 6 | Sources read once per `check` | done |
| 7 | 128-bit cache key with a bounded store | done |
| 8 | Semantic-tier cache entry | dropped — see item 6 |
| 9 | CI gates over what CI can reach | done |
| 10 | Language cleanup; three items kept, with reasons | done |
