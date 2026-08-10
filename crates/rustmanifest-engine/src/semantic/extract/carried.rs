// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The unit tests an extraction carries along: a test that exercises
//! the moved function follows it into the new file's own test block.
//!
//! A test moves only when the move is provably whole: its tokens
//! mention the moved function, and mention no other name of the old
//! file's top level and no sibling inside the test module — a test
//! resting on anything left behind stays behind, silently. The carried
//! block re-imports the parent with a glob, copies the `use` lines the
//! moved tests mention from both the test module and the file top, and
//! an emptied test module is deleted whole.

use rustmanifest_schema::Range;

use super::{
    names::{bound_names, mentioned_idents},
    ranged, swallowed
};

/// What the extraction carries: the test block for the new file and
/// the deletions from the old one.
pub(super) struct Carried {
    /// The `#[cfg(test)]` block for the new file, or nothing to carry.
    pub(super) block: Option<String>,
    /// Deletions from the old file: the moved tests, or the emptied
    /// test module whole.
    pub(super) edits: Vec<(Range, String)>
}

/// Plans the tests carried with the function named `moved`, given the
/// names of everything else the old file declares at top level.
pub(super) fn carried_tests(
    text: &str,
    parsed: &syn::File,
    moved: &str,
    siblings: &[String]
) -> Carried {
    let Some((module, content)) = test_module(parsed) else {
        return Carried {
            block: None,
            edits: Vec::new()
        };
    };
    let local: Vec<String> = content
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(held) => Some(held.sig.ident.to_string()),
            _ => None
        })
        .collect();
    let mut carried = Vec::new();
    let mut carried_mentions = std::collections::BTreeSet::new();
    for item in content {
        let syn::Item::Fn(held) = item else {
            continue;
        };
        let name = held.sig.ident.to_string();
        let span = lined(text, ranged(syn::spanned::Spanned::span(held)));
        let Some(body) = text.get(span.start as usize..span.end as usize) else {
            continue;
        };
        let mentions = mentioned_idents(body);
        let whole = mentions.contains(moved)
            && !siblings.iter().any(|sibling| mentions.contains(sibling))
            && !local
                .iter()
                .filter(|other| *other != &name)
                .any(|other| mentions.contains(other));
        if whole {
            carried.push((span, body.to_owned()));
            carried_mentions.extend(mentions);
        }
    }
    if carried.is_empty() {
        return Carried {
            block: None,
            edits: Vec::new()
        };
    }
    let mut lines = vec!["#[cfg(test)]".to_owned(), "mod tests {".to_owned()];
    lines.push("    use super::*;".to_owned());
    for used in module_uses(text, parsed, content) {
        let bound = bound_names(&used.1);
        let wanted = bound.is_empty() || bound.iter().any(|name| carried_mentions.contains(name));
        if wanted && used.0.trim() != "use super::*;" {
            lines.push(format!("    {}", used.0.trim()));
        }
    }
    lines.push(String::new());
    for (position, (_, body)) in carried.iter().enumerate() {
        if position > 0 {
            lines.push(String::new());
        }
        lines.push(body.trim_end().to_owned());
    }
    lines.push("}".to_owned());
    let emptied = carried.len() == local.len();
    let edits = if emptied {
        vec![(
            swallowed(text, ranged(syn::spanned::Spanned::span(module))),
            String::new()
        )]
    } else {
        carried
            .iter()
            .map(|(span, _)| (swallowed(text, *span), String::new()))
            .collect()
    };
    Carried {
        block: Some(lines.join("\n")),
        edits
    }
}

/// The one inline `#[cfg(test)]` module of the file, with its items.
fn test_module(parsed: &syn::File) -> Option<(&syn::ItemMod, &[syn::Item])> {
    parsed.items.iter().find_map(|item| match item {
        syn::Item::Mod(held) if under_test_cfg(&held.attrs) => held
            .content
            .as_ref()
            .map(|(_, items)| (held, items.as_slice())),
        _ => None
    })
}

/// Whether the attributes carry `#[cfg(test)]`.
fn under_test_cfg(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|inner| inner == "test")
    })
}

/// The written `use` items of the test module and of the file top,
/// each with its parsed tree — the vocabulary a carried test may need.
fn module_uses<'a>(
    text: &'a str,
    parsed: &'a syn::File,
    content: &'a [syn::Item]
) -> Vec<(&'a str, syn::UseTree)> {
    content
        .iter()
        .chain(parsed.items.iter())
        .filter_map(|item| match item {
            syn::Item::Use(used) => {
                let span = ranged(syn::spanned::Spanned::span(used));
                text.get(span.start as usize..span.end as usize)
                    .map(|written| (written, used.tree.clone()))
            }
            _ => None
        })
        .collect()
}

/// The range widened to the start of its line, so a carried test keeps
/// its indentation.
fn lined(text: &str, range: Range) -> Range {
    Range {
        start: super::line_start(text, range.start),
        end:   range.end
    }
}
