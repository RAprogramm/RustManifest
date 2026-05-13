# rustmanifest-cli

Command-line frontend for the `rustmanifest` engine.

```
rustmanifest check <path>
rustmanifest review-diff <revspec>
rustmanifest explain <rule-id>
rustmanifest init
```

Outputs JSON, SARIF 2.1.0, or human-readable TTY. Used directly from local shells, pre-commit hooks, and CI.

Phase 0 ships only the binary skeleton with `--version` and the subcommand surface locked in.
