<!--
SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
SPDX-License-Identifier: MIT
-->

# RFC 0004 — What a program takes in, computes, and gives back

| Field | Value |
|-------|-------|
| Status | Final |
| Author | @RAprogramm |
| Created | 2026-08-04 |
| Last updated | 2026-08-06 |
| Affected crates | `rustmanifest-engine`, `rustmanifest-cli` |
| SemVer impact | minor (four new fact channels, one new tier, new report surface; existing channels unchanged) |

**Status note (2026-08-06).** All eight stages shipped, recorded as Phase 2V in
[the roadmap](../ROADMAP.md). Two things landed differently from what is written
below and the roadmap states them there: the method rows of stage 6 ship as
standard-library knowledge only, so a conversion performed by an ecosystem pack is
not yet recorded as one; and closure gained a third ground — every variant of a
standard type named — which is weaker than a catch-all, says so, and does not read
a scrutinee's type. Everything the `Unresolved questions` section lists is still
open.

## Summary

The engine can state what a program writes and what status it leaves. It cannot
state what a program reads, what it computes, or where a written value came from.
This RFC adds the value half of the fact base: an inward boundary channel mirroring
the outward one, operations and choices as recorded sites, handbacks, and — the
load-bearing artefact — a per-function summary that lets the origin of a value
cross a call.

The acceptance target is not a sentence. It is a derivation: from an ordinary
command-line calculator the engine derives, with a trace whose every step is a
recognizer or a `file:line`, which values the program takes from outside itself,
what it computes from them, and what it hands back — refusing, by name, wherever
the source does not settle the question.

## Motivation

Every prescription the engine can make today (`prescribe`, the four `RM-IO-*`
forms) is about *how* a write is spelled, never about *what* it says, because
nothing links a written position to a value. The meaning tier says
`"error: <error>"` and, since RFC 0003's follow-on, can name the texts that
position's own type renders — but that answer was reached by a special case
(a type's display implementation), and a ladder of special cases has no top.

The general question behind all of them is where a value came from. Answering it
once, as a primitive, subsumes the special cases already shipped and yields the
statements none of them can reach: that a value was drawn from the command line,
converted to a number by a call the library states can fail, and combined with
another such value by an arithmetic operation chosen by a third.

The project's stated end goal is that the engine writes ideal code and explains
what and why, with facts as the substrate for gated generation. A generator cannot
be gated on facts describing only the outward half of a program.

## Decisions

### D1 — The function summary is the load-bearing artefact

For one function: where the value it hands back comes from, written in a
vocabulary whose only free variables are that function's own parameter slots.
Because the only free variables are slots, a summary is **substitutable** — at a
call site the caller replaces slot `k` with the origin of its own argument `k` and
is left with a term naming nothing of the callee's. That property, and nothing
else, is why provenance crosses a call. Everything else in this RFC either feeds
the summary or speaks it.

### D2 — Four new channels, built exactly like `output_writes`

One record per site, in body order, never deduplicated, each carrying its
`SiteGuard`, each with its own versioned recognizer-set constant so that an empty
list reads as "the recognizer found none".

| Channel | Records |
|---|---|
| `input_reads` | a value drawn from outside the process |
| `operations` | arithmetic, comparison, selection, a library conversion |
| `choices` | a construct choosing among written alternatives |
| `handbacks` | a site handing a value back to the caller, `?` included |

`CallSite` is enriched with the arguments it hands over, the disposal written
around it, its byte range, and the conditions standing over it.

### D3 — A value reference inside one body is an index, not a subtree

`Provenance` names a record of the owning function by channel and position. Two
consumers of one value name the same record instead of carrying two copies. The
tree exists only in the composition tier, bounded there — the same split the
fact/statement pair `OutputWrite` / `OutputClaim` already draws.

### D4 — Conditions ride beside the guard stack, and `Guard` is not touched

The guard stack records the *kind* of an enclosing construct; a parallel condition
stack records *which* one, one entry per frame, outermost first, with
`conditions.len() == guard.guards.len()` as an invariant. This yields arm identity
— the gap `io_forms/lock_run.rs` documents in prose — without changing `Guard`,
`SiteGuard`, `guard_name`'s `const fn(Guard) -> &'static str` contract, or a single
golden file.

### D5 — Refusals are named, per question, at the leaf

`OriginGap` names the recognizer question that failed: `Unreadable`, `Rebound`,
`Foreign`, `Unwritten`, `CalleeUnresolved`, `CalleeExternal`, `Recursion`,
`Dispatched`, `Deeper`, `Unenumerated`. A gap is never read as an absence of the
thing it failed on.

### D6 — What this tier will not claim, settled up front

- **No disjunction between unrelated failure sites.** Two positions under two arms
  of *one recorded choice* are proven alternatives and may be joined by "or".
  Everything else is listed. A sentence joining "a reading fails" with "the divisor
  is zero" is not derivable and no stage may earn it.
- **No overflow behaviour.** Whether a bare `+` traps is fixed by `overflow-checks`,
  and `StdFact` has no field naming the configuration a library fact holds under.
  Integer arithmetic records `Guarding::Unchecked` and the sentence says the build
  decides. It never says `Panicked`, which is false under `--release`.
- **No rules tier.** Nothing is added to `io_forms` or `check.rs`. `Form::properties`
  is hard-typed to the output subject and `std_derive` has derived no property of
  `Stdin`, of `str::parse`, or of any arithmetic. Derive first, catalogue later.
- **No type inference.** A binding whose type is decided by a later use records
  `produced: None`, permanently, at this tier.

## Detailed design

### The value term — `behavior/value.rs` + `value/`

```rust
pub enum Provenance {
    /// A value written in the source, kept as written and never evaluated.
    Written(Literal),
    /// The value one of this function's own inputs arrived as. The one free
    /// variable of the term, and the whole of why a summary composes.
    Parameter(ParamSlot),
    /// The value the read at this position of `input_reads` draws.
    Read(u32),
    /// The value the operation at this position of `operations` produces.
    Computed(u32),
    /// Whichever value the alternative taken by the choice at this position
    /// of `choices` produces.
    Chosen(u32),
    /// The value the call at this position of `call_sites` hands back.
    Called(u32),
    /// Not derived, with the reason.
    NotDerived(OriginGap)
}
```

`ParamSlot` counts every input the signature declares, the receiver first, so a
call site's written arguments align with it. It is deliberately **not** a position
in `FunctionFacts::params`: that list drops an input it cannot profile — a receiver
with no impl type, a pattern that is not a plain name, a type whose last segment
does not render — so its positions align with nothing. `FunctionFacts` therefore
gains `slots: Vec<Option<String>>`, one entry per declared input.

Supporting types: `Literal` (digits kept as written), `Arguments::{Written,
Unwritten}` — `Unwritten` is not hypothetical, the collector synthesizes call sites
for the `?` error conversion, for an `await`'s poll, and for a service wrapper's
inner call — `Disposal::{Propagated, Panicked, Defaulted, Examined, Infallible,
NotIdentified}`, and `Condition::{Branch, Arm, NotDerived}`.

### The inward boundary — `behavior/input.rs` + `input/`

Field-for-field mirror of `OutputWrite`: `channel↔stream`, `selector↔payload`,
`terminator↔newline`, `intake↔conduit`, `acquisition↔acquisition`, plus `shape` and
`conditions`. The mirror is claimed for this record and for no other; the interior
channels inherit the *discipline*, not the field list.

```rust
pub enum Channel { CommandLine, Environment, StandardInput, CallerSupplied, Unknown }
pub enum Selector { Whole, Positional(u32), Named(String), NotIdentified(SelectorGap) }
pub enum Intake { Text, Raw }          // args validates UTF-8; args_os does not
pub enum Terminator { Retained, Stripped, None }  // read_line keeps it; lines strips it
pub enum ReadShape { Sequence, Element, Text }
```

`Acquisition` is reused verbatim: `STREAM_GUARD_SUFFIX` already answers for
`StdinLock`.

### Operations and choices

```rust
pub enum Operation { Arithmetic(Arithmetic), Comparison(Comparison), Conversion(Conversion), Selection }
pub enum Guarding { Unchecked, Checked, Saturating, Wrapping, Reported }
```

Language operators stay a `const fn` over `syn::BinOp`/`syn::UnOp`: an operator is
syntax, not library semantics, and the "as data, not code branches" rule is about
the library. Library methods that perform an operation live in a table
(`checked_add` → `Add`/`Checked`, `saturating_add` → `Add`/`Saturating`, …); a
method absent from it is not read as an operation, so a namesake of the program's
own is never credited with the library's behaviour.

A `Choice` records its scrutinee, one `Alternative` per written arm — pattern as
written, `irrefutable` and `conditional` read by the same `pattern::{irrefutable,
arm_condition}` the guard frame already uses, so the two readings of one arm can
never disagree — and a `Closure` that is `CatchAll` only where an irrefutable
unconditional alternative exists. A `match` over an enum with one arm per variant
and no catch-all records `Unproven`: nothing in the fact base enumerates a type's
variants and this tier does not pretend to.

### Recognizer tables, all as data

Read sources are matched on the **whole expanded path** — `["std","env","args"]`,
`["std","env","var"]`, `["std","io","stdin"]` — for the reason `STREAM_HANDLE_PATHS`
already states: a bare `var` would credit a configuration crate's own lookup to the
process environment. `env!` and `option_env!` are deliberately absent: they read the
environment of the build, not of the process.

A boundary adapter table carries a second column saying whether the adapter keeps
positions. It is why `env::args().collect()` is still a read of the command line —
without it the boundary is lost at the first adapter, since the receiver of
`collect` types to nothing today. An adapter that does not keep positions degrades
the selector to `NotIdentified(Advanced)` while the channel stays proven.

Conversions enter through the knowledge packs: `CrateKnowledge` gains
`method_results: &'static [MethodResult]`, where a row states what a method hands
back (`ReturnForm::{TypeArgument, Receiver, Element, Named}`) and whether it can
fail (`Outcome::{Total, Fallible, Optional}`). Lookup order at a site is **workspace
table, then pack, then std** — the order the call-type reading already uses.

### The collector

New state: `origins` (binding name → `Provenance`, block-scoped), `sole` (names
bound or assigned exactly once), `slots`, `conditions`.

The single-binding census counts, per name, every `visit_pat_ident` occurrence
**and** every bare-ident target of an assignment **and** every bare-ident left
operand of a compound assignment. The last two are mandatory: the field-write
reading handles only `Expr::Field`, so a bare `x = 1` records nothing on any
channel, and a census built on patterns alone would call `let mut t = 0; t += x;`
singly bound and state a first-iteration origin for a value the loop rewrites.

`origin_of(&syn::Expr) -> Provenance` is total and **records nothing** — it is a
lookup over state the walk has already built plus the index of a record the walk
has already pushed. Every site is recorded exactly once, by the walk, at the node
that owns it. Borrows, parentheses and groups are seen through; a field, a static
or a captured name is `Foreign`.

Disposal is a property of the enclosing construct, which is visited before its
operand, so the reading is a post-hoc stamp with an exact test: note the channel
lengths, recurse, then find among the records added by that recursion the one whose
byte range **equals** the disposed expression's own. Exactly one → stamp it. Zero
or several → `NotIdentified`. Outcome adapters (`map_err`, `ok_or`, `ok_or_else`)
are seen through first; `map` is not, since it changes the value and seeing through
it would attribute a disposal to a different value than the one disposed.

Four new visitor overrides are needed: `visit_expr_unary`, `visit_expr_cast`,
`visit_expr_index`, `visit_expr_return`. The multiplicity stamp, which today
hardcodes the write channel, is generalized to stamp every per-site channel.

### The composition tier — `semantic/summary.rs` + `summary/`

Tarjan condensation over the call graph emits each component only after every
component it reaches, so emission order is already callee-first: **one pass, no
fixpoint, no convergence argument**. A callee inside the caller's own component
yields `NotDerived(Recursion)` per origin leaf, not per function, so a caller that
also calls a settled helper keeps that helper's composed origin.

Per fact, in emission order: fold `handbacks` into one term (none →
`NotDerived(Unreadable)`; one → its origin; several → a `Choice`, `closed` false
whenever any handback stands under a guard); then at every call settle what it
carries — dispatched over several implementors → `Dispatched`; unbound candidate →
a knowledge row if one answers, else `CalleeUnresolved`/`CalleeExternal`; same
component → `Recursion`; otherwise substitute the site's arguments for the callee's
slots.

`unfold` decrements a depth budget at every constructor and emits
`NotDerived(Deeper)` at zero, so termination does not rest on any acyclicity
argument.

The binding rule (`sole_by_name`, `bound_callee`) moves out of `meaning/reach.rs`
and is shared, so grading and composition never disagree about what a candidate
names, and the trace says which rule it used. The stricter resolver is chosen
deliberately over the call graph's own fallbacks: a summary crediting a caller with
a callee bound by module-qualifier fallback would state more than the site proves.

**One normalization rule**, stated and deterministic: a `Selection` whose first
operand is a boundary sequence with `Selector::Whole` and whose second is a written
integer `k` is a read of that channel at position `k`. `Selector::Whole` in the
premise is load-bearing — it is set only when every adapter in the chain kept
positions. The rule restates two proven facts as one node; it infers nothing.

### Claims and sentences

`IntakeClaim`, `ComputeClaim`, `ChoiceClaim` and `HandbackClaim` are field for
field `OutputClaim`, with `execution = reach.min(guard.local_grade())` unchanged.
Every recorded site becomes a claim, unreached ones included. `EntryMeaning` gains
them in boundary order and keeps `claims` as the write list — renaming it for
symmetry would break every consumer for a cosmetic gain.

The compound statement decomposes rather than weakens:

```rust
pub enum Narrative {
    Whole(String),
    Parts { refused: (Clause, OriginGap), sentences: Vec<String> }
}
```

**The licensing rule.** A clause is licensed when no fact it rests on is a refusal
at a load-bearing position. A *grade* below `Certain` does **not** refuse: it
degrades the wording and names the weakener, exactly as the write sentence already
does. Requiring `Certain` for the whole would make the compound statement
unreachable for any program whose reads sit after a `?` — which is every program of
this shape.

## The derivation, worked

For a three-argument calculator whose `main` collects `env::args()`, reads two
values through a helper that selects an element and converts it, chooses on the
third among five written alternatives, prints the result and returns a result type,
the engine derives: the command line is read once as a sequence; three selections
over it normalize to positions 1, 2 and 3; two of them feed a conversion whose
knowledge row states it can fail; the choice has five alternatives of which one
accepts every remaining value, so the set is closed; four of the alternatives name
an arithmetic operation over the two converted values; the write's value is the
call the choice produces; four recorded sites hand back a failure, of which exactly
two stand on two alternatives of one recorded choice and are therefore alternatives
of each other, while the other two are listed and not joined.

**Grades are measured, not assumed.** A `let` whose initializer carries `?` raises
the prior-exit count, so on this program the second and third reads and the write
are `Possible`, not `Certain`, and the sentence says "may". Any acceptance test
asserting "unconditionally" for this shape asserts a falsehood.

## Refusals

| Where | Term |
|---|---|
| name bound or assigned more than once | `Rebound` |
| expression shape outside the vocabulary | `Unreadable` |
| a field, a static, a captured name | `Foreign` |
| candidate matching several definitions, or none | `CalleeUnresolved` |
| candidate outside the workspace with no knowledge row | `CalleeExternal` |
| callee in the caller's own recursion component | `Recursion` |
| dispatch over several implementors | `Dispatched` |
| site's arguments not written anywhere | `Unwritten` |
| past the depth budget | `Deeper` |
| more alternatives than the tier enumerates, or a handback under a guard | `Unenumerated` / `closed: false` |
| adapter outside the table, or one not keeping positions | `Selector::NotIdentified(Advanced)`, channel still proven |
| conversion target written nowhere | `Conversion { target: None }` |
| disposed range matching zero or several new records | `Disposal::NotIdentified` |
| integer arithmetic | `Guarding::Unchecked`, never `Panicked` |
| no catch-all alternative | `Closure::Unproven` |
| `return` inside a closure or nested item | not recorded — it leaves that body, not this one |
| a positional format hole | `NotDerived(Unreadable)`, on the ground the type reading already refuses there |
| any clause resting on a refusal | `Narrative::Parts` |
| every empty channel | its own versioned negative |

Five recognizer-set constants — reads, operations, choices, origins, summaries —
are stamped on every claim and named in every negative, each hand-maintained as
`OUTPUT_SINK_SET` is. Deriving one from a hash of its tables was considered and
rejected: it would change on an unrelated reordering and stop meaning "the
recognizer this claim came from".

## Stages

Each stage compiles, passes the suite, ships a statement the engine could not
previously make, and leaves no half-populated field. The interlock is the compiler:
`FunctionFacts` is built by two exhaustive struct literals with no
`..Default::default()`, so a new channel fails to build until both initialize it.

| Stage | Content |
|---|---|
| 0 | The return-type fix: a nameless success type must keep its place ahead of the failure type. Ships alone. |
| 1 | The value term, slots, handbacks, call-site arguments. |
| 2 | The summary tier: condensation, callee-first single pass, substitution, the six composition refusals. |
| 3 | The condition stack beside the guard stack. |
| 4 | The inward boundary. |
| 5 | Operations and choices. |
| 6 | Conversions as knowledge rows. |
| 7 | The compound statement. |

Every stage that changes a fact lands its golden regeneration as its own commit
immediately after the code commit, so a reviewer can tell a fact change from a
rendering change.

## Drawbacks

**Accumulating loops are outside this tier.** `let mut sum = 0; for x in xs { sum
+= x; }` yields `NotDerived(Rebound)` for every later read of `sum` — not "computes
something unknown", but "the origin is not derived because the name is written more
than once". The vocabulary is therefore far wider than the coverage, and a per-entry
count of answered versus refused positions belongs in the report's honesty section
so a reader does not infer breadth from the shape of the type.

**Two name environments with different scope rules.** The type table is flat and
monotone and other readings depend on that ordering; `origins` is block-scoped and
refuses on rebinding. A format hole can therefore be typed through a shadowed
binding while its origin refuses. Both readings are individually honest; unifying
them is a separate change with its own blast radius.

**Pack method lookup is unscoped by crate.** Method rows are matched on a bare last
type segment across every pack. `parse`, `add`, `get` and `from_str` are among the
most-shadowed names in the language. The mitigation is the existing order —
workspace answers first — but a workspace type whose name collides with a row's
owner *and* whose method the tables cannot type will take the row. The gap exists
today; this RFC makes it more load-bearing.

**Exhaustiveness is provable only through a catch-all**, so the engine is *less*
certain about the better-written `match`. Closing it needs an enum-variant channel,
which is small and is not in this RFC.

**No exit record for a `?`.** The completion channel recognizes only the four exit
spellings, so an entry failing through `?` yields no exit claim. The failure clause
is derived instead from the disposal plus the entry's declared completion, and it
names the mechanism at each site rather than a process-ending site, because there is
none.

**The depth and alternative budgets are prescription.** Nothing in the library or
the analyzed program licenses their values, exactly as the canonical output forms
admit that the shape is this repository's prose while the property is the library's.

## Alternatives

**A one-hop composition rule** — compose a caller with its immediate callees and
refuse beyond — was proposed and rejected: a two-level helper chain, which is the
ordinary shape of the target program, then yields nothing at all.

**A recursive origin tree stored in the fact base** was proposed and rejected: it
duplicates a subtree at every consumer of a value, where an index does not.

**Encoding arm identity inside `Guard`** was proposed and rejected: it changes a
`const fn(Guard) -> &'static str` export contract and the equality of every
recorded guard stack, for a fact the parallel condition stack carries for free.

## Unresolved questions

- A configuration qualifier on `StdFact`, so `panic-on-overflow:debug` becomes
  statable and arithmetic can enter the canonical-form catalogue.
- A resolved-crate field on a receiver type, so pack rows stop firing on workspace
  namesakes.
- An enum-variant channel, so exhaustiveness stops requiring a catch-all.
- A recognizer stamp in the engine-level JSON, which carries none today and which
  this RFC widens fivefold without fixing.
