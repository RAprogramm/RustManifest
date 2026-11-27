# RFC 0005 — Migrate LSP server to tower-lsp-server

## Status

Proposed.

## Motivation

The current LSP crate uses `lsp-server 0.10` + `lsp-types 0.97`. In 2026
the maintained fork is `tower-lsp-server` (community fork of `tower-lsp`),
which tracks LSP 3.18 and uses `gen-lsp-types`. The unmaintained `lsp-types`
crate lags behind the spec.

## Proposal

Migrate `rustmanifest-lsp` to `tower-lsp-server` + `gen-lsp-types`, gaining
inline completion, ranges formatting, and workspace text document content
support.

## Risks

- API churn in trait methods.
- Need to re-test all LSP handlers.

## Decision

Deferred until after the masterror migration and tier-3 orchestrator land.