<!--
SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
SPDX-License-Identifier: MIT
-->

# RFC 0002 — Meaning tier: framework models, external surface, subsystem capabilities

| Field | Value |
|-------|-------|
| Status | Final |
| Author | @RAprogramm |
| Created | 2026-08-03 |
| Last updated | 2026-08-06 |
| Affected crates | `rustmanifest-engine`, `rustmanifest-cli`, `rustmanifest-schema` |
| SemVer impact | minor (new engine surface, new report sections; no rule-schema change) |

## Summary

Teach the engine what a program *is for*, not only how it is built: recover the
external contract a service exposes (method, path, handler, request and response
types), the capability of every subsystem (serves a socket, reads configuration,
reaches storage), and the composition rooted at the entry point. Recovery is
deterministic and evidence-carrying — every recovered row names the call that
produced it — and rests on **framework models encoded as data**, the same shape as
the existing knowledge packs.

## Motivation

The semantic tier (RFC 0001) answers *structure* and *behavior*: which modules exist,
who writes which field, which functions reach the outside world and along what call
chain. It cannot answer the question a reviewer actually starts from:

> The entry point starts a server. What does that server expose, what does each
> endpoint need, what does it hand back, and which subsystem does the work?

That information is not missing from the code — it is *declared*, in the framework's
registration API. In the corpus on this machine the declarations are plain typed
method chains over string literals and handler identifiers:

```rust
Router::new()
    .route("/api/analytics/pageview", post(track_page_view_handler))
    .route("/api/analytics/metrics", get(get_metrics_handler))
```

and the contract of each handler is its signature:

```rust
pub async fn track_page_view_handler(
    claims: Claims,
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<TrackPageViewRequest>
) -> AppResult<(StatusCode, Json<PageViewResponse>)>
```

No macro expansion, no reflection, no configuration file. Method, path, handler,
authentication marker, request body type and response type are all present as typed
syntax. Recovering them is extraction, not inference.

## Prior art

Framework-aware analysis is an established requirement, not a novelty: general
static analysis is blind to framework-dispatched entry points, so analyzers ship a
model of the framework itself. Jasmine models Spring annotations and XML wiring to
recover controller entry points; FlowDroid models Android lifecycle callbacks; SARL
proposes a dedicated language for specifying framework behavior so an analyzer can
consume it as data. The recurring lesson is that the model belongs in data, kept
next to the analyzer and grown per framework, rather than in special-cased code.

Architecture-recovery work published in 2026 converges on a second point: keep
deterministic extraction strictly separated from any semantic synthesis layer, or
reproducibility is lost. This project takes the stronger position already recorded
in its governance — there is no synthesis layer at all. Anything the extractor
cannot prove is reported as unknown.

## Design

### 1. Framework model as data

A framework model extends the existing `CrateKnowledge` pack shape with the
registration vocabulary of the crate:

| Entry | Meaning |
|---|---|
| `router_constructors` | calls that produce a router value (`Router::new`) |
| `route_registrations` | methods that attach a route to a router (`route`) |
| `method_wrappers` | calls naming the HTTP method of a handler (`get`, `post`, `put`, `patch`, `delete`, `head`, `options`) |
| `composition` | methods that fold one router into another (`nest` with a path prefix, `merge` without) |
| `extractors` | types whose appearance in a handler signature means a typed input (`Path`, `Query`, `Json`, `Form`, `Extension`, `State`) |
| `listeners` | constructors that bind a listening socket, separating a server from a client |

The model states what the framework's own documentation guarantees. A construct not
in the model produces no row — silence, never a guess.

### 2. Route recovery

For every function body, method-call chains rooted at a router constructor are
walked. A `route` registration contributes one row per method wrapper:
`(method, path, handler)` with the byte range of the registration. Composition is
resolved through the existing call graph: a function whose return type is a router
is a *router provider*, and `nest("/prefix", provider())` prefixes every row the
provider contributes, while `merge(provider())` contributes them unchanged. A
composition argument that is not a resolvable provider call yields an explicit
`unresolved composition` row rather than a silently missing subtree.

### 3. Handler contract

The handler's signature is read against the model's extractor table: a parameter
typed `Json<T>` is the request body, `Path<T>` the path parameters, `Query<T>` the
query string, `State<S>` / `Extension<S>` the shared state, and any other type that
appears as a bare parameter is an application-defined extractor — recorded by name,
since it usually carries authentication. The return type yields the response payload
by unwrapping the project's result alias down to the framework's response types.

### 4. Subsystem capability

Per module, the engine already knows the effect set of every function. The meaning
tier turns that into a capability with evidence: a module whose functions bind a
listening socket *serves*; one that reaches the database *stores*; one that reads
the environment *configures*. Each capability names the function and the call that
proved it. The entry point's reachable set, taken from the call graph, states which
subsystems the program actually starts.

## Honesty gates

- Ambiguity is dropped, never guessed — the same gate the call resolver obeys.
- A path assembled at runtime (a loop over a configuration table, a computed string)
  is not recoverable. Such registrations are counted and reported as dynamic, so a
  reader can see the recovered table is partial.
- Macro-generated routers stay invisible to the syntactic pass and need the existing
  expansion mode; the report says which crates were expanded.
- Every row carries file, byte range, and the producing call.

## Acceptance

Recovery is measurable against ground truth the projects themselves declare: crates
annotated for OpenAPI generation list their handler paths independently of the router
chain. Precision and recall of the recovered route table are computed against that
declaration on real projects, in the same style as the SCIP ground-truth experiment
(research doc §9). A recovery that cannot beat the declared table on either measure
is not shipped.

## Alternatives rejected

- **Parsing the generated OpenAPI document.** It only exists where the project opted
  into it, requires a build, and describes intent rather than the code that runs.
- **Name and directory heuristics** (`handlers.rs` is handlers, `domains/` are
  domains). Cheap and wrong on the first project that names things differently; it
  also violates the standing rule that understanding is derived from facts.
- **A learned classifier over module features.** The learning layer exists and stays
  descriptive by design; a route table must be exact, not probable.
