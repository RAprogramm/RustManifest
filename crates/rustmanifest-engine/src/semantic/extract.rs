// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The extraction plan: one top-level function moved verbatim into its
//! own module file, with everything the move mechanically implies.
//!
//! The new file lands where the module convention puts it — a folder
//! named after the current file, with the crate roots staying flat —
//! and carries the function's text unchanged, the imports whose bound
//! names its tokens mention, and a `super` import for the same-file
//! items it references, which privacy already lets a child module see.
//! A private function gains `pub(super)` so the parent keeps reaching
//! it; the function's place is left clean — the `mod` line lands among
//! the file's `mod` declarations in alphabetical order, or at the top
//! where none exist, and the re-export lands at the imports — so every
//! existing call keeps compiling; and the imports the remaining text no
//! longer mentions are deleted whole. Methods of `impl` blocks are
//! refused: moving a method is a different operation than moving a free
//! function.

mod carried;
mod names;
mod typed;

use std::path::PathBuf;

use rustmanifest_schema::Range;

use self::names::{bound_names, mentioned_idents};
pub use self::typed::extract_type;
use crate::source::Source;

/// One planned extraction: the file to create and the edits to the
/// file the function leaves, each edit a byte range with the text that
/// replaces it — an empty text deletes, an empty range inserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// Path of the module file to create.
    pub new_file: PathBuf,
    /// Whole text of the new file.
    pub new_text: String,
    /// Edits to the old file, in no particular order.
    pub edits:    Vec<(Range, String)>
}

/// Plans the extraction of the top-level function covering `offset` in
/// source `file`, or nothing where no whole free function stands there.
#[must_use]
pub fn extract_function(sources: &[Source], file: usize, offset: u32) -> Option<Extraction> {
    let source = sources.get(file)?;
    let text = source.text();
    let parsed = syn::parse_file(text).ok()?;
    let function = parsed.items.iter().find_map(|item| match item {
        syn::Item::Fn(held) => {
            let span = ranged(syn::spanned::Spanned::span(held));
            ((span.start..span.end).contains(&offset)).then_some((held, span))
        }
        _ => None
    });
    let (function, span) = function?;
    let name = function.sig.ident.to_string();
    let body = text.get(span.start as usize..span.end as usize)?;
    let mentions = mentioned_idents(body);
    let mut lines = Vec::new();
    for item in &parsed.items {
        if let syn::Item::Use(used) = item {
            let bound = bound_names(&used.tree);
            let wanted = bound.is_empty() || bound.iter().any(|name| mentions.contains(name));
            if wanted && let Some(written) = written_of(text, used) {
                lines.push(written.trim_end().to_owned());
            }
        }
    }
    let siblings = sibling_names(&parsed, &name);
    let bridged: Vec<&String> = siblings
        .iter()
        .filter(|sibling| mentions.contains(sibling.as_str()))
        .collect();
    if !bridged.is_empty() {
        let named: Vec<&str> = bridged.iter().map(|held| held.as_str()).collect();
        lines.push(format!("use super::{{{}}};", named.join(", ")));
    }
    let public = !matches!(function.vis, syn::Visibility::Inherited);
    let moved = if public {
        body.to_owned()
    } else {
        promoted(body, span.start, &function.sig)?
    };
    let mut new_text = if lines.is_empty() {
        format!("{moved}\n")
    } else {
        format!("{}\n\n{moved}\n", lines.join("\n"))
    };
    let followed = carried::carried_tests(text, &parsed, &name, &siblings);
    if let Some(block) = &followed.block {
        new_text.push('\n');
        new_text.push_str(block);
        new_text.push('\n');
    }
    let mut edits: Vec<(Range, String)> = vec![(swallowed(text, span), String::new())];
    edits.extend(followed.edits);
    let mod_line = format!("mod {name};\n");
    let reexport = if public {
        format!("pub use self::{name}::{name};\n")
    } else {
        format!("use self::{name}::{name};\n")
    };
    edits.extend(mounted(text, &parsed, &name, mod_line, reexport));
    for removed in stale_imports(text, &parsed, &[span]) {
        edits.push((swallowed(text, removed), String::new()));
    }
    Some(Extraction {
        new_file: module_file(source.path(), &name)?,
        new_text,
        edits: disjoint(edits)
    })
}

/// The insertions mounting the new module in the old file: the `mod`
/// line at its alphabetical slot and the re-export at the imports,
/// combined into one where both land at the same offset.
fn mounted(
    text: &str,
    parsed: &syn::File,
    slot_name: &str,
    mod_line: String,
    reexport: String
) -> Vec<(Range, String)> {
    let mod_at = mod_slot(text, parsed, slot_name);
    let use_at = parsed
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Use(used) => Some(line_start(
                text,
                ranged(syn::spanned::Spanned::span(used)).start
            )),
            _ => None
        })
        .unwrap_or(mod_at);
    if use_at == mod_at {
        vec![(point(mod_at), format!("{mod_line}{reexport}"))]
    } else {
        vec![(point(mod_at), mod_line), (point(use_at), reexport)]
    }
}

/// The edits sorted and clamped so no two ranges overlap: two widened
/// deletions may both have swallowed the same blank line, and an
/// applier owes each byte to at most one edit.
fn disjoint(mut edits: Vec<(Range, String)>) -> Vec<(Range, String)> {
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let mut settled: Vec<(Range, String)> = Vec::new();
    for (mut range, replacement) in edits {
        if let Some((last, _)) = settled.last()
            && range.start < last.end
        {
            range.start = last.end;
            if range.end < range.start {
                range.end = range.start;
            }
        }
        if range.start == range.end && replacement.is_empty() {
            continue;
        }
        settled.push((range, replacement));
    }
    settled
}

/// The insertion point of the `mod` line: its alphabetical slot among
/// the file's `mod` declarations, or the top of the items where the
/// file declares none. Inline `mod` blocks do not count — a test block
/// at the bottom of a file is not the declaration list.
fn mod_slot(text: &str, parsed: &syn::File, name: &str) -> u32 {
    let declared: Vec<(String, Range)> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(held) if held.content.is_none() => Some((
                held.ident.to_string(),
                ranged(syn::spanned::Spanned::span(held))
            )),
            _ => None
        })
        .collect();
    if let Some((_, span)) = declared
        .iter()
        .find(|(declared, _)| declared.as_str() > name)
    {
        return line_start(text, span.start);
    }
    if let Some((_, span)) = declared.last() {
        return next_line_start(text, span.end);
    }
    parsed.items.first().map_or(0, |item| {
        line_start(text, ranged(syn::spanned::Spanned::span(item)).start)
    })
}

/// The deleted range widened over the blank line the deletion leaves;
/// a deletion reaching the end of the file also takes the blank line
/// standing before it, so the file keeps a single final newline.
fn swallowed(text: &str, range: Range) -> Range {
    let mut end = range.end as usize;
    let mut eaten = 0;
    while eaten < 2 && text.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
        eaten += 1;
    }
    let mut start = range.start as usize;
    if end == text.len() {
        while start > 1 && text.as_bytes().get(start - 1) == Some(&b'\n') && {
            text.as_bytes().get(start.wrapping_sub(2)) == Some(&b'\n')
        } {
            start -= 1;
        }
    }
    Range {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end:   u32::try_from(end).unwrap_or(u32::MAX)
    }
}

/// The byte offset where the line holding `at` begins.
fn line_start(text: &str, at: u32) -> u32 {
    let capped = (at as usize).min(text.len());
    let begun = text[..capped].rfind('\n').map_or(0, |found| found + 1);
    u32::try_from(begun).unwrap_or(u32::MAX)
}

/// The byte offset where the line after `at` begins.
fn next_line_start(text: &str, at: u32) -> u32 {
    let capped = (at as usize).min(text.len());
    let begun = text[capped..]
        .find('\n')
        .map_or(text.len(), |found| capped + found + 1);
    u32::try_from(begun).unwrap_or(u32::MAX)
}

/// An empty range standing at one offset: an insertion point.
const fn point(at: u32) -> Range {
    Range {
        start: at,
        end:   at
    }
}

/// The module file the convention names: a folder after the current
/// file's stem, with the crate roots staying flat beside themselves.
fn module_file(path: &std::path::Path, name: &str) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let parent = path.parent()?;
    let dir = if matches!(stem, "lib" | "main" | "mod") {
        parent.to_path_buf()
    } else {
        parent.join(stem)
    };
    Some(dir.join(format!("{name}.rs")))
}

/// The function text with `pub(super)` written before the `fn` keyword.
fn promoted(body: &str, from: u32, sig: &syn::Signature) -> Option<String> {
    let at = ranged(sig.fn_token.span).start.checked_sub(from)? as usize;
    let mut held = body.to_owned();
    if held.len() < at {
        return None;
    }
    held.insert_str(at, "pub(super) ");
    Some(held)
}

/// Top-level names of the file besides the moved function: what a
/// `use super` bridge can carry.
fn sibling_names(parsed: &syn::File, moved: &str) -> Vec<String> {
    parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(held) => Some(held.sig.ident.to_string()),
            syn::Item::Struct(held) => Some(held.ident.to_string()),
            syn::Item::Enum(held) => Some(held.ident.to_string()),
            syn::Item::Const(held) => Some(held.ident.to_string()),
            syn::Item::Static(held) => Some(held.ident.to_string()),
            syn::Item::Trait(held) => Some(held.ident.to_string()),
            syn::Item::Type(held) => Some(held.ident.to_string()),
            _ => None
        })
        .filter(|name| name != moved)
        .collect()
}

/// Whole `use` items with no mention left once the moved pieces are
/// gone: every bound name absent from the remaining non-import text.
fn stale_imports(text: &str, parsed: &syn::File, gone: &[Range]) -> Vec<Range> {
    let mut remaining = String::new();
    let mut cuts: Vec<Range> = gone.to_vec();
    for item in &parsed.items {
        if let syn::Item::Use(used) = item {
            cuts.push(ranged(syn::spanned::Spanned::span(used)));
        }
    }
    cuts.sort_by_key(|range| range.start);
    let mut cursor = 0usize;
    for cut in &cuts {
        if let Some(kept) = text.get(cursor..cut.start as usize) {
            remaining.push_str(kept);
        }
        cursor = cut.end as usize;
    }
    remaining.push_str(text.get(cursor..).unwrap_or(""));
    let mentions = mentioned_idents(&remaining);
    parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Use(used) => {
                let bound = bound_names(&used.tree);
                (!bound.is_empty() && bound.iter().all(|name| !mentions.contains(name)))
                    .then(|| ranged(syn::spanned::Spanned::span(used)))
            }
            _ => None
        })
        .collect()
}

/// The written text of one `use` item.
fn written_of<'a>(text: &'a str, used: &syn::ItemUse) -> Option<&'a str> {
    let span = ranged(syn::spanned::Spanned::span(used));
    text.get(span.start as usize..span.end as usize)
}

/// The schema range of one span.
fn ranged(span: proc_macro2::Span) -> Range {
    let bytes = span.byte_range();
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Extraction, extract_function};
    use crate::source::Source;

    fn planned(path: &str, text: &str, inside: &str) -> Option<Extraction> {
        let sources = [Source::new(PathBuf::from(path), text.to_owned())];
        let offset = u32::try_from(text.find(inside).unwrap()).unwrap() + 1;
        extract_function(&sources, 0, offset)
    }

    fn applied(text: &str, plan: &Extraction) -> String {
        let mut edits = plan.edits.clone();
        edits.sort_by_key(|(range, _)| std::cmp::Reverse((range.start, range.end)));
        let mut held = text.to_owned();
        for (range, inserted) in edits {
            held.replace_range(range.start as usize..range.end as usize, &inserted);
        }
        held
    }

    const TASKS: &str = "use std::collections::BTreeMap;\nuse std::collections::HashSet;\nuse \
                         std::fmt::Write as _;\n\nfn keep(store: &BTreeMap<u64, String>) -> \
                         usize { store.len() }\n\nfn render(store: &BTreeMap<u64, String>) -> \
                         String {\n    let mut seen: HashSet<u64> = HashSet::new();\n    \
                         seen.insert(0);\n    let mut out = String::new();\n    write!(out, \
                         \"{}\", keep(store)).ok();\n    out\n}\n";

    #[test]
    fn the_new_file_lands_by_the_module_convention() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert_eq!(plan.new_file, PathBuf::from("src/tasks/render.rs"));
        let flat = planned("src/main.rs", TASKS, "fn render").unwrap();
        assert_eq!(flat.new_file, PathBuf::from("src/render.rs"));
    }

    #[test]
    fn the_moved_text_carries_its_imports_and_the_super_bridge() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert!(plan.new_text.contains("use std::collections::BTreeMap;"));
        assert!(plan.new_text.contains("use std::fmt::Write as _;"));
        assert!(plan.new_text.contains("use super::{keep};"));
        assert!(plan.new_text.contains("pub(super) fn render"));
    }

    #[test]
    fn the_function_leaves_no_residue_and_the_mod_line_lands_on_top() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert_eq!(
            applied(TASKS, &plan),
            "mod render;\nuse self::render::render;\nuse std::collections::BTreeMap;\nuse \
             std::fmt::Write as _;\n\nfn keep(store: &BTreeMap<u64, String>) -> usize { \
             store.len() }\n"
        );
    }

    #[test]
    fn the_mod_line_takes_its_alphabetical_slot_among_declarations() {
        let text = "mod alpha;\nmod zulu;\n\nfn render() -> u8 { 1 }\n\nfn keep() -> u8 { \
                    render() }\n";
        let plan = planned("src/tasks.rs", text, "fn render").unwrap();
        assert_eq!(
            applied(text, &plan),
            "mod alpha;\nmod render;\nuse self::render::render;\nmod zulu;\n\nfn keep() -> u8 { \
             render() }\n"
        );
    }

    /// A nameless import (`as _`, a glob) is copied to the new file and
    /// never deleted from the old one: no mention can prove it unused,
    /// and a silent copy is a smaller claim than a broken build.
    #[test]
    fn a_nameless_import_is_kept_on_both_sides() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert!(plan.new_text.contains("use std::fmt::Write as _;"));
        assert!(applied(TASKS, &plan).contains("use std::fmt::Write as _;"));
    }

    #[test]
    fn a_public_function_keeps_its_visibility_and_re_exports_publicly() {
        let text = "pub fn shout() -> u8 { 1 }\nfn keep() -> u8 { shout() }\n";
        let plan = planned("src/tasks.rs", text, "fn shout").unwrap();
        assert!(plan.new_text.contains("pub fn shout"));
        assert!(!plan.new_text.contains("pub(super)"));
        assert_eq!(
            applied(text, &plan),
            "mod shout;\npub use self::shout::shout;\nfn keep() -> u8 { shout() }\n"
        );
    }

    #[test]
    fn a_cursor_outside_any_function_plans_nothing() {
        assert!(planned("src/tasks.rs", TASKS, "use std::collections").is_none());
    }

    #[test]
    fn an_impl_method_is_refused() {
        let text = "struct Disk;\nimpl Disk { fn spin(&self) {} }\n";
        assert!(planned("src/tasks.rs", text, "fn spin").is_none());
    }

    const TESTED: &str = "fn keep() -> u8 { 1 }\n\nfn render() -> u8 { keep(); 2 }\n\n\
                          #[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn \
                          renders() {\n        assert_eq!(render(), 2);\n    }\n\n    #[test]\n    \
                          fn keeps() {\n        assert_eq!(keep(), 1);\n    }\n}\n";

    #[test]
    fn a_test_of_the_moved_function_follows_it() {
        let plan = planned("src/tasks.rs", TESTED, "fn render").unwrap();
        assert!(plan.new_text.contains("#[cfg(test)]"), "{}", plan.new_text);
        assert!(plan.new_text.contains("fn renders()"), "{}", plan.new_text);
        assert!(!plan.new_text.contains("fn keeps()"), "{}", plan.new_text);
        let left = applied(TESTED, &plan);
        assert!(!left.contains("fn renders()"), "{left}");
        assert!(left.contains("fn keeps()"), "{left}");
    }

    #[test]
    fn a_test_resting_on_a_left_behind_sibling_stays_behind() {
        let text = "fn keep() -> u8 { 1 }\n\nfn render() -> u8 { 2 }\n\n#[cfg(test)]\nmod tests \
                    {\n    use super::*;\n\n    #[test]\n    fn both() {\n        \
                    assert_eq!(render(), keep() + 1);\n    }\n}\n";
        let plan = planned("src/tasks.rs", text, "fn render").unwrap();
        assert!(!plan.new_text.contains("#[cfg(test)]"), "{}", plan.new_text);
        assert!(applied(text, &plan).contains("fn both()"));
    }

    const TYPED: &str = "use std::collections::BTreeMap;\n\nfn count(user: &UserStore) -> usize \
                         { user.entries().len() }\n\nstruct UserStore {\n    held: BTreeMap<u64, \
                         String>,\n}\n\nimpl UserStore {\n    fn entries(&self) -> &BTreeMap<u64, \
                         String> {\n        &self.held\n    }\n}\n\nimpl std::fmt::Display for \
                         UserStore {\n    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> \
                         std::fmt::Result {\n        write!(out, \"{}\", self.held.len())\n    \
                         }\n}\n";

    #[test]
    fn a_type_moves_with_its_impls_into_a_snake_named_file() {
        let sources = [Source::new(PathBuf::from("src/tasks.rs"), TYPED.to_owned())];
        let offset = u32::try_from(TYPED.find("struct UserStore").unwrap()).unwrap() + 1;
        let plan = super::extract_type(&sources, 0, offset).unwrap();
        assert_eq!(plan.new_file, PathBuf::from("src/tasks/user_store.rs"));
        assert!(
            plan.new_text.contains("pub(super) struct UserStore"),
            "{}",
            plan.new_text
        );
        assert!(
            plan.new_text.contains("impl UserStore {"),
            "{}",
            plan.new_text
        );
        assert!(
            plan.new_text
                .contains("impl std::fmt::Display for UserStore {"),
            "{}",
            plan.new_text
        );
        assert!(
            plan.new_text.contains("use std::collections::BTreeMap;"),
            "{}",
            plan.new_text
        );
        assert!(
            plan.new_text.contains("pub(super) fn entries"),
            "the method the staying file calls must widen: {}",
            plan.new_text
        );
        let left = applied(TYPED, &plan);
        assert!(
            left.contains("mod user_store;\nuse self::user_store::UserStore;"),
            "{left}"
        );
        assert!(!left.contains("struct UserStore"), "{left}");
        assert!(!left.contains("impl"), "{left}");
        assert!(left.contains("fn count"), "{left}");
        assert!(
            !left.contains("use std::collections::BTreeMap;"),
            "the staying file no longer mentions the map, so the import must go: {left}"
        );
    }

    #[test]
    fn an_emptied_test_module_is_deleted_whole() {
        let text = "fn render() -> u8 { 2 }\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    \
                    #[test]\n    fn renders() {\n        assert_eq!(render(), 2);\n    }\n}\n";
        let plan = planned("src/tasks.rs", text, "fn render").unwrap();
        let left = applied(text, &plan);
        assert!(!left.contains("mod tests"), "{left}");
        assert!(plan.new_text.contains("fn renders()"));
    }
}
