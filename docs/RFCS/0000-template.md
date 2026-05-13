# RFC NNNN — <short title>

| Field | Value |
|-------|-------|
| Status | Draft / Active / Final / Withdrawn / Rejected |
| Author | @your-handle |
| Created | YYYY-MM-DD |
| Last updated | YYYY-MM-DD |
| Affected crates | e.g. `rustmanifest-engine`, `rustmanifest-rules-core` |
| SemVer impact | none / minor / major |

## Summary

One paragraph. What is this RFC proposing in plain language.

## Motivation

Why does this need to exist. What problem does it solve. What constraint is it trying to relax. Use concrete examples — link to issues, real user reports, or a specific failure mode.

## Detailed design

The bulk of the RFC. Be precise enough that another contributor could implement it from this document alone:

- Public API or schema changes (Rust signatures, JSON Schema deltas).
- Rule definitions: IDs, default severities, fixture pass/fail examples.
- Transport or protocol-level changes: wire format, error codes, auth.
- Migration story for users.
- Default behavior and feature-flag gating.

## Drawbacks

What is the cost of this change. Maintenance burden. Performance impact. Surface-area growth. Security implications.

## Rationale and alternatives

What other designs were considered. Why is this design chosen over them. What is the impact of not doing this.

## Prior art

Linked references — other lint engines, MCP servers, language tools that have solved similar problems. Cite what worked and what did not.

## Unresolved questions

Concrete questions that must be answered before the RFC can move from Draft to Active. Each question should be answerable; vague aspirations belong in `Future possibilities`.

## Future possibilities

What this RFC unblocks but does not commit to. Out-of-scope ideas that follow naturally.

## Implementation plan

Stepwise plan. Which PRs land first. What gates each step. Who owns each step.
