<!-- SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com> -->
<!-- SPDX-License-Identifier: MIT -->

# masterror migration gaps (P1.7)

Observed during RustManifest P1 migration to `masterror 0.32.0`.

## Upstream issues

- `RAprogramm/masterror#491`: `From<io::Error> for AppError` loses
  `io::ErrorKind` and source chain. With `std`, conversion maps `io::Error` to
  `AppErrorKind::Internal` with `err.to_string()` as public message; original
  `io::ErrorKind` is not preserved and original `io::Error` is not attached as
  `source`.
- `RAprogramm/masterror#492`: `ResultExt` has no anyhow-style lazy
  `with_context` message. `ResultExt::context(msg)` takes
  `impl Into<Cow<'static, str>>` eagerly; `ResultExt::ctx` builds
  `masterror::Context`, not a lazy message, and `Context::into_error` leaves
  message unset.

## RustManifest notes

- `masterror::ResultExt::context` wraps source and preserves kind / code /
  metadata for existing `AppError`; source-attaching context and `anyhow`
  `with_context(|| ...)` migration still require explicit mapping.
- `From<serde_json::Error> for AppError` exists in `masterror` with the
  `serde_json` feature; RustManifest uses `masterror` with
  `default-features = false`, so JSON errors use `ResultExt::context` or domain
  errors.
- Orphan rules prevent downstream crates from implementing
  `From<foreign::Error> for masterror::AppError`; `masterror` can only provide
  common conversions in the crate defining `AppError`.
- `#[derive(Error)]` / `#[derive(Masterror)]` reduce manual `Display` / `Error`
  / `From<domain> for AppError` boilerplate; RustManifest keeps manual impls
  where explicit control is needed.

## Action

Upstream problems are tracked in `RAprogramm/masterror` separate issues.
RustManifest P1 is not blocked on them.
