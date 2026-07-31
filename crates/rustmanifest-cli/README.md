# rustmanifest-cli

Command-line frontend for the `rustmanifest` engine.

```
rustmanifest check <path>
rustmanifest review-diff <revspec>
rustmanifest explain <rule-id>
rustmanifest init
```

Outputs JSON, SARIF 2.1.0, or human-readable TTY. Used directly from local shells, pre-commit hooks, and CI.

## Clippy delegation

`rustmanifest check --delegate-clippy <path>` additionally runs `cargo clippy --message-format=json --quiet` when `<path>` contains a `Cargo.toml` (the probe is shallow; parent workspace manifests are not searched). Clippy diagnostics are surfaced as **advisory** findings under rustmanifest's reporting umbrella: the rule id is the full clippy code (e.g. `clippy::clone_on_copy`), the severity is clippy's own (`error` → error, `warning` → warning, anything else → info), and the rationale URI links to the corresponding entry in the [clippy lint index](https://rust-lang.github.io/rust-clippy/master/index.html). Delegation is best-effort: a missing `cargo`, a failed invocation, or malformed output lines only produce warnings and never fail the run.

Phase 0 ships only the binary skeleton with `--version` and the subcommand surface locked in.
