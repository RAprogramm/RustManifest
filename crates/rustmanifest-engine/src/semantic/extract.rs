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
//! it; the removed body becomes the `mod` line and a re-export in
//! place, so every existing call keeps compiling; and the imports the
//! remaining text no longer mentions are deleted whole. Methods of
//! `impl` blocks are refused: moving a method is a different operation
//! than moving a free function.

mod names;

use std::path::PathBuf;

use rustmanifest_schema::Range;

use self::names::{bound_names, mentioned_idents};
use crate::source::Source;

/// One planned extraction: the file to create and the edits to the
/// file the function leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// Path of the module file to create.
    pub new_file:        PathBuf,
    /// Whole text of the new file.
    pub new_text:        String,
    /// Byte range of the moved function in the old file.
    pub replaced:        Range,
    /// What stands in its place: the `mod` line and the re-export.
    pub replacement:     String,
    /// Whole `use` items of the old file left without a single mention
    /// once the function is gone, in text order.
    pub removed_imports: Vec<Range>
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
    let siblings = sibling_names(&parsed, &name)
        .into_iter()
        .filter(|sibling| mentions.contains(sibling))
        .collect::<Vec<_>>();
    if !siblings.is_empty() {
        lines.push(format!("use super::{{{}}};", siblings.join(", ")));
    }
    let public = !matches!(function.vis, syn::Visibility::Inherited);
    let moved = if public {
        body.to_owned()
    } else {
        promoted(body, span.start, &function.sig)?
    };
    let new_text = if lines.is_empty() {
        format!("{moved}\n")
    } else {
        format!("{}\n\n{moved}\n", lines.join("\n"))
    };
    let replacement = if public {
        format!("mod {name};\npub use self::{name}::{name};")
    } else {
        format!("mod {name};\nuse self::{name}::{name};")
    };
    Some(Extraction {
        new_file: module_file(source.path(), &name)?,
        new_text,
        replaced: span,
        replacement,
        removed_imports: stale_imports(text, &parsed, span)
    })
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

/// Whole `use` items with no mention left once the function is gone:
/// every bound name absent from the remaining non-import text.
fn stale_imports(text: &str, parsed: &syn::File, gone: Range) -> Vec<Range> {
    let mut remaining = String::new();
    let mut cuts: Vec<Range> = vec![gone];
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
    fn the_old_file_gets_the_mod_line_and_the_re_export_in_place() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert_eq!(plan.replacement, "mod render;\nuse self::render::render;");
        let replaced = &TASKS[plan.replaced.start as usize..plan.replaced.end as usize];
        assert!(replaced.starts_with("fn render"));
        assert!(replaced.ends_with('}'));
    }

    #[test]
    fn an_import_only_the_moved_function_used_is_deleted_whole() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert_eq!(plan.removed_imports.len(), 1);
        let removed =
            &TASKS[plan.removed_imports[0].start as usize..plan.removed_imports[0].end as usize];
        assert_eq!(removed, "use std::collections::HashSet;");
    }

    /// A nameless import (`as _`, a glob) is copied to the new file and
    /// never deleted from the old one: no mention can prove it unused,
    /// and a silent copy is a smaller claim than a broken build.
    #[test]
    fn a_nameless_import_is_kept_on_both_sides() {
        let plan = planned("src/tasks.rs", TASKS, "fn render").unwrap();
        assert!(plan.new_text.contains("use std::fmt::Write as _;"));
        assert!(
            !plan
                .removed_imports
                .iter()
                .map(|range| &TASKS[range.start as usize..range.end as usize])
                .any(|removed| removed == "use std::fmt::Write as _;")
        );
    }

    #[test]
    fn a_public_function_keeps_its_visibility_and_re_exports_publicly() {
        let text = "pub fn shout() -> u8 { 1 }\nfn keep() -> u8 { shout() }\n";
        let plan = planned("src/tasks.rs", text, "fn shout").unwrap();
        assert!(plan.new_text.contains("pub fn shout"));
        assert!(!plan.new_text.contains("pub(super)"));
        assert_eq!(plan.replacement, "mod shout;\npub use self::shout::shout;");
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
}
