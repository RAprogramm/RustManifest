<!--
SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
SPDX-License-Identifier: MIT
-->

# Design: documentation as facts

Goal: a fully deterministic system that both **checks** and **drafts**
rustdoc documentation to community best practices (docs.rs readability),
derived from logical facts only. No model inside the loop; a model may
optionally polish a finished draft outside the engine, and its text is a
suggestion, never a fact.

## 1. Doc-fact extractor

Parse `///` / `//!` into structured claims per item:

- summary line: first sentence, leading verb, length;
- declared sections: `# Errors`, `# Panics`, `# Safety`, `# Examples`;
- identifiers mentioned in backticks (parameter and type references).

## 2. Truth rules (claimed vs observed)

Join doc claims with behavioral facts; every contradiction is a finding
with a derivation trace:

- `# Errors` declared, but the signature returns no `Result` — or the
  reverse direction is clippy's (`missing_errors_doc`); **presence is
  clippy's job, truth is ours** — no duplication;
- body provably can panic (`unwrap`/`expect`/indexing/`panic!`) while the
  doc declares `# Panics` about nothing, or vice versa;
- doc mentions a parameter name absent from the signature (stale doc);
- summary verb contradicts behavior (says "Returns", body writes state) —
  same syllogism engine as `deceptive-name` (RM-QUAL-001).

## 3. Deterministic doc synthesizer

Facts → community-formula draft, templates are hand-written English with
fact slots; grammar is guaranteed by construction:

- summary from verb + object + role ("Returns the current balance.");
- `# Errors` enumerated from `error_types`;
- `# Panics` from detected panic sites;
- `# Safety` from the `unsafe` effect;
- argument lines from parameter ownership modes.

The docs.rs standard is formulaic enough that templates cover the bulk;
what remains is polish, which stays outside the engine by the
no-LLM-in-understanding constraint.

## Order of implementation

1. extractor (`semantic/docs.rs`): doc facts beside behavior facts;
2. truth rules: `RM-QUAL-002` (stale parameter mention), `RM-QUAL-003`
   (panic documented vs proven) — each gated by corpus review like
   RM-QUAL-001;
3. synthesizer: `doc-draft` surface producing ready-to-paste rustdoc.
