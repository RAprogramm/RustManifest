<!-- SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com> -->
<!-- SPDX-License-Identifier: MIT -->

# masterror migration gaps (P1.7)

Observed during RustManifest P1 migration to `masterror 0.32.0`.

## Gaps / friction

- **Foreign error conversions in downstream crates.** A downstream crate cannot
  implement `From<foreign::Error> for masterror::AppError` (orphan rule).
  `masterror` already provides `From<std::io::Error> for AppError`, but not
  `From<serde_json::Error> for AppError`; `serde_json::Error` converts to
  `masterror::Error`, not `AppError`. RustManifest maps foreign errors through
  domain errors (`ReportError::from(e)`, `ResolveError::from(e)`),
  `masterror::ResultExt::context`, or `AppError::with_context`. A
  `ResultExt::context_with_source` style helper, or documented
  `From<serde_json::Error> for AppError`, would reduce boilerplate.
- **Domain error boilerplate.** Manual `Display`/`Error`/`From<domain> for
  AppError` implementations are repetitive for small enums. `#[derive(Error)]`
  / `#[derive(Masterror)]` reduce this, but RustManifest keeps manual impls
  where explicit control is needed.
- **`ResultExt::context` vs source attachment.** `ResultExt::context(msg)`
  converts to `AppError`, preserving kind / code / metadata for existing
  `AppError`; attaching the original error as a source uses
  `AppError::with_context` / `map_err`. `ResultExt` has no
  `with_context`-closure form like `anyhow::Context::with_context`. An
  ergonomic "context + keep source" result-extension would reduce mapping code.
- **`anyhow::Context` migration.** `masterror::ResultExt` must be imported;
  `context` takes `impl Into<Cow<'static, str>>`, not `FnOnce() -> ...`.
  Call sites on foreign errors still need explicit mapping when no
  `From<E> for AppError` conversion exists.

## Action

Track upstream in masterror as separate PRs/issues when the API shape is
agreed. Do not block RustManifest P1 on these gaps.
