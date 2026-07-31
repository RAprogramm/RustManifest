# rustmanifest-eval

Evaluation harness for the `rustmanifest` engine: runs the full rule set
(pattern, AST, and semantic tiers) over a pinned corpus of repositories and
turns the resulting finding counts into a regression gate.

The corpus is described by `eval/corpus.toml` at the workspace root. Each
`[[repo]]` entry names a repository, its checkout path relative to a corpus
root, the pinned git revision, and a short note. Locally, existing checkouts
under the corpus root are used as-is; in CI the pinned revisions would be
cloned fresh, which makes the counts exactly reproducible. When a local
checkout has drifted from its pin, the harness warns, records the actual
revision in the report, and keeps going.

## Subcommands

```
rustmanifest-eval run   --corpus-root <dir> [--out eval/report.json]
rustmanifest-eval check --corpus-root <dir> [--baseline eval/baseline.json]
rustmanifest-eval bless --corpus-root <dir>
```

- `run` evaluates every corpus entry and writes a full JSON report: per-repo
  per-rule finding counts, file and line totals, the actual checkout
  revision, and a `timings` section with the p95 per-file analysis time over
  files under 1000 lines.
- `check` produces the same report in memory and compares its *comparable
  section* (everything except `actual-rev` and `timings`) against the
  committed baseline. Any per-rule per-repo count difference is listed, and
  the command exits `1` when any count changed or when any repository's p95
  over files under 1000 lines exceeds the 100 ms budget. Exit `0` otherwise;
  operational failures exit `2`.
- `bless` overwrites `eval/baseline.json` with the current comparable
  section.

Timings are deliberately excluded from the baseline: wall-clock numbers vary
between machines and runs, so `check` evaluates them only against the fixed
budget and never diffs them.

## Baseline workflow

1. `rustmanifest-eval bless --corpus-root <dir>` — record the current counts.
2. Commit `eval/baseline.json` together with the change that produced it.
3. `rustmanifest-eval check --corpus-root <dir>` — the regression gate; run
   it locally and in CI. A failing check means either an intended rule
   change (re-bless and commit) or an unintended regression (fix it).
