# rustmanifest-cli

Command-line frontend for the `rustmanifest` engine.

```
rustmanifest check <path>
rustmanifest coverage <path>
rustmanifest explain <rule-id>
rustmanifest facts <path>
rustmanifest graph <path>
rustmanifest learn <path>
rustmanifest overview <path>
```

`rustmanifest overview <path>` prints everything the engine knows about the project as one deterministic Markdown report: size and understanding honesty, the module map with cycle warnings, the most connected types, the behavioral role distribution, outside-world boundary functions, the most contended state fields, and the conventions the learning layer extracted.

Outputs JSON, SARIF 2.1.0, or human-readable TTY. Used directly from local shells, pre-commit hooks, and CI. Exit codes: `0` clean, `1` findings at or above the severity filter, `2` operational error.

`rustmanifest coverage <path>` reports how much of the analyzed sources the engine actually understands (with `--expand` measuring derive/macro output through `cargo +nightly rustc -Zunpretty=expanded`). `rustmanifest facts <path>` dumps behavioral facts of every function as JSON (`--filter`, `--active`).

## Architecture diagrams

`rustmanifest graph <path>` prints a dependency diagram of the analyzed sources to stdout. `--kind modules` (default) draws the intra-crate module dependency graph, highlighting modules that participate in a dependency cycle; `--kind types` draws structure aggregation edges labeled with the aggregating field. `--format` selects `mermaid` (default) or `dot`. Files are discovered exactly like `check` (gitignore-aware, `.rs` only) and nodes and edges are sorted, so output is deterministic and paste-ready for Markdown or Graphviz.

```mermaid
graph LR
    crate["crate"]
    crate_util["crate::util"]
    crate_util --> crate
```

## Clippy delegation

`rustmanifest check --delegate-clippy <path>` additionally runs `cargo clippy --message-format=json --quiet` when `<path>` contains a `Cargo.toml` (the probe is shallow; parent workspace manifests are not searched). Clippy diagnostics are surfaced as **advisory** findings under rustmanifest's reporting umbrella: the rule id is the full clippy code (e.g. `clippy::clone_on_copy`), the severity is clippy's own (`error` → error, `warning` → warning, anything else → info), and the rationale URI links to the corresponding entry in the [clippy lint index](https://rust-lang.github.io/rust-clippy/master/index.html). Delegation is best-effort: a missing `cargo`, a failed invocation, or malformed output lines only produce warnings and never fail the run.
