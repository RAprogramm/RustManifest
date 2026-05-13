<!--
SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
SPDX-License-Identifier: MIT
-->

## Summary

<!-- One paragraph. What changes, why now. Link to the issue or RFC. -->

## Type of change

- [ ] Bug fix
- [ ] New feature (non-rule)
- [ ] New rule
- [ ] Documentation
- [ ] Refactor (no behavior change)
- [ ] CI / infrastructure
- [ ] Breaking change

## Checklist

- [ ] `cargo +nightly fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- [ ] `cargo test --workspace` passes
- [ ] `reuse lint` passes (every new file carries SPDX headers or is annotated in `REUSE.toml`)
- [ ] `cargo deny check` passes
- [ ] Schema drift gate passes (or schemas regenerated and committed)
- [ ] Public API additions have rustdoc; changes are covered by tests or fixtures
- [ ] For new rules: pass/fail fixtures committed, methodology section linked
- [ ] For breaking changes: deprecation entry in `docs/GOVERNANCE.md`

## Related

<!-- Closes #N — or — Part of RFC NNNN -->
