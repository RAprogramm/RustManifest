<!--
SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
SPDX-License-Identifier: MIT
-->

# RFC 0003 — Observable output: what a program writes, to which stream, and whether it is certain

| Field | Value |
|-------|-------|
| Status | Final |
| Author | @RAprogramm |
| Created | 2026-08-03 |
| Last updated | 2026-08-06 |
| Affected crates | `rustmanifest-engine`, `rustmanifest-cli`, `rustmanifest-mcp` |
| SemVer impact | minor (new fact channel, new report surface; existing channels unchanged) |

## Summary

Give the engine the smallest complete statement of what a program *does* for an
observer outside the process: which byte payload it writes, to which standard
stream, from which entry point, and whether that write is certain or merely
possible. The acceptance target is one sentence about the smallest possible Rust
program — *"at start the process unconditionally writes the string `Hello, world!`
to its standard output stream"* — with every word of it backed by a recorded fact.

## Motivation

The engine records a single coarse category for this today: an effect named
`console`, set when the collector sees a print-family macro. That category answers
"something is printed" and nothing else. It cannot say **which stream** (standard
output and standard error are different observable channels), **what** is written
(the payload is discarded), **whether** the write actually happens (a write inside
an `if` and a write in a straight-line body are indistinguishable), or **in what
order** (the effect list is sorted and deduplicated, so two writes collapse into
one).

A program stated to print a greeting that does not print it is broken. To make that
statement checkable at all, the fact base must first be able to express it.

## Decisions

### D1 — the coarse effect category is not split; a new channel is added

The obvious move is to replace the `console` category with two, one per stream. It
is rejected, and the reason is structural rather than cosmetic: the effect list is a
**sorted, deduplicated set**. It cannot carry order, multiplicity, payload, or the
guard a write sits under — precisely the four things the target sentence needs. A
split buys a stream label that the new record carries anyway, and costs the
binarized feature vector its fixed length and vocabulary, the learning layer its
reproducible output at a fixed seed, every user-declared `effect = "console"` ban
its silent match, the committed JSON schema a regeneration, and the eval corpus its
byte-stable baseline.

This is the project's own standing rule, first forced by the pointer-transparency
drift and recorded in the research doc: **a new fact shape gets a new channel; it
does not deform an existing one.** The precise fact lives in the new channel. The
coarse category stays as a summary label, and retiring or renaming it is a separate,
separately-baselined step — not a side effect of this work.

### D2 — two guarantee axes, never merged into a score

`execution` is a local, must-style property: is this write reached on every path
through its own function. `reach` is an interprocedural, may-style property: is the
owning function reached from the entry point, and how certainly. They compose
(`min`), but they are recorded separately with separate evidence. No confidence
number is produced anywhere — a number would hide which of the two is weak.

### D3 — the payload models what the format machinery itself guarantees

A payload is `Empty`, a `Literal`, a `Template` of proven text segments interleaved
with holes, or `Opaque` with a reason. `fully_known` returns a string only when every
segment is proven text — the same predicate the standard library's own formatting
arguments expose. A hole is never rendered as a value, never as an empty string, and
the argument expression behind it is kept as provenance, never evaluated.

### D4 — a stream has four values, not two

`Stdout`, `Stderr`, `CallerSupplied` (a write into a generic writer the caller
provides), `Unknown`. Recording a site whose channel cannot be proven is the honest
form: omitting it would make "no recognized write" indistinguishable from "writes
nothing". For the same reason every claim carries the version of the recognizer list
that produced it, so absence is reported as *the recognizer found none*, not as
*there are none*.

### D5 — build targets are not touched

Entry-point identification gets its own predicate over file layout and attributes.
Widening the existing build-target notion would change call-graph linkability, and
therefore coverage and the corpus baseline, for no gain to this fact.

### D6 — the existing channels stay frozen while the new one lands

Two known weaknesses — a closure body donating its effects to the enclosing function
although the closure is never called, and macro arguments never being visited so the
calls inside them are invisible — are real and are *not* fixed in the same steps.
Each moves the baseline in its own direction and is done alone, with the moved counts
named in the commit.

## Shape of the fact

Per function, an insertion-ordered, never-deduplicated list of write sites. Each
site: stream, payload, whether a newline is appended, the byte range that proves it,
the guard stack it sits under (branch, loop, closure, nested item, configuration
gate), how many exits precede it in the body, and its multiplicity (once, bounded,
unbounded).

Per entry point, a claim per site: the sentence the recorded fields license, degraded
field by field rather than collapsed into a single hedge — a weakened certainty names
the guard that weakened it, a partially known payload prints its proven text with the
holes marked as holes, an unidentifiable stream says so.

## Admitted limits

- Exhaustiveness is judged syntactically: an `if`/`else` chain with a final `else`
  and a `match` with a wildcard arm are exhaustive; a `match` covering every variant
  of an enum without a wildcard is not, and stays *possible*.
- Configuration gates are not resolved — the engine reads no manifest, so a write
  under a feature gate is *possible*, never *certain*.
- The standard library can divert print-family output through its capture hook, and a
  failed write panics rather than writing. Every claim about a standard stream is
  therefore about an *attempted* write; this caveat is attached to the claim, not
  hidden in documentation.
- Writes through raw file descriptors or foreign functions are outside the recognizer
  set.
