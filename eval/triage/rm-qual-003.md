# RM-QUAL-003 triage: the five extended-corpus findings

The orphan-file rule was silent on the four-repo corpus (degenerate
precision: 0 true over 0 false) and spoke on the extended corpus with
five findings — aura-api 4, ratma-studio-api 1. This protocol reviews
each finding against the trees at the checkouts the corpus pins, so
the rule's precision on the extended corpus is a measured number
rather than a finding counter.

Method, per finding: confirm the file resolves from no crate root
(read the named parent's declarations), rule out invisible mounts
(search the tree for `#[path]` and `include!` naming the file), search
the tree for any reference to the file's items, and classify what the
orphan is — a forgotten mount or an abandoned leftover.

## aura-api

**1. `src/domains/aura_event/infrastructure/sql.rs` — true.**
The parent `infrastructure.rs` declares five modules and no `mod sql;`.
The file holds eight `include_str!` constants over `.sql` files that
all exist beside it, so mounting it would compile — but no code in the
tree references any of the constants, and the repository layer runs 39
inline query calls instead. The orphan is an abandoned start of a
move-SQL-to-constants migration, not a forgotten mount.

**2–4. `src/domains/user/infrastructure/sql/{create,get,update}.rs` —
true, all three.** Here the parent `sql.rs` *is* mounted and holds the
flat include-constants form; the three files are a second, deeper
decomposition attempt (per-operation submodules with their own child
files under `create/` and `get/`) that was never mounted and is
referenced nowhere. One detail proves the fix's `MaybeIncorrect`
marking right: `update.rs` declares two children whose directory does
not exist, so the suggested `mod update;` alone would not even
compile — whether to finish the refactor or delete it is a human
decision, exactly what the marking says. The rule reported only the
three subtree roots, not their child files: mounting a root heals its
subtree, so one finding per root is the minimal true answer.

## ratma-studio-api

**5. `src/utils/generate_presigned_url.rs` — true.** The parent
`utils.rs` declares seventeen modules and not this one; nothing in the
tree references the function; the live upload path goes through the
mounted `media_storage` and `upload_image` modules. A dead leftover.

## Verdict

Precision on the extended corpus: 5 true, 0 false. The degenerate-
precision question is closed by data: the rule speaks when there is
something to say and its findings survive review. The humility guard's
silence on the 17 macro-wrapped hyper files stands unchanged. All five
orphans are abandoned code rather than forgotten mounts — the fix is
still right to offer the mount and still right to mark it
`MaybeIncorrect`, because in four of five cases the correct human
action is deletion, and in one the mount would not compile.
