// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Process entry points: which function the operating system starts.
//!
//! A binary target's root file declares one entry function. It is either
//! a plain `main`, or an `async main` wrapped by a runtime attribute
//! that generates the real entry around it. Both are recognized from
//! syntax and file layout alone.
//!
//! Everything ambiguous is refused rather than assumed: a `main` behind
//! a configuration gate is recorded as conditional, a `main` nested in a
//! module is not an entry point at all, and a crate that declares no
//! entry has none — a library is not a program.

use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Runtime attributes that generate a real entry around an `async main`,
/// matched on the last two path segments so a renamed import still
/// resolves.
const RUNTIME_ENTRY_ATTRS: [(&str, &str); 6] = [
    ("actix_rt", "main"),
    ("actix_web", "main"),
    ("async_std", "main"),
    ("monoio", "main"),
    ("smol_potat", "main"),
    ("tokio", "main")
];

/// How a function becomes the process entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A plain `fn main` in a binary root.
    Plain,
    /// An `async fn main` wrapped by a runtime attribute.
    Runtime,
    /// A `main` behind a configuration gate this tier does not resolve;
    /// whether it is the entry depends on the build.
    Conditional
}

/// One recognized process entry point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPoint {
    /// Index of the fact describing the entry function.
    pub fact: usize,
    /// How it becomes the entry.
    pub kind: EntryKind,
    /// Path of the binary root file that declares it.
    pub file: String
}

/// Finds every process entry point among `sources`.
///
/// A file is a binary root when it is the crate's `main.rs`, a file
/// directly under a `bin` directory, or the `main.rs` of a directory
/// under `bin` or `examples`. Only top-level functions of such a file
/// are considered, so a `main` inside a module is not an entry.
#[must_use]
pub fn entry_points(sources: &[Source], facts: &[FunctionFacts]) -> Vec<EntryPoint> {
    let mut entries = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let path = source.path().to_string_lossy().replace('\\', "/");
        if !binary_root(&path) {
            continue;
        }
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        for item in &file.items {
            let syn::Item::Fn(function) = item else {
                continue;
            };
            if function.sig.ident != "main" {
                continue;
            }
            let Some(kind) = entry_kind(function) else {
                continue;
            };
            let Some(fact) = facts
                .iter()
                .position(|entry| entry.source == index && entry.function == "main")
            else {
                continue;
            };
            entries.push(EntryPoint {
                fact,
                kind,
                file: path.clone()
            });
        }
    }
    entries
}

/// Returns whether a path names a file that compiles into a binary.
fn binary_root(path: &str) -> bool {
    let components: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let last = components.last().copied().unwrap_or_default();
    let parent = components
        .len()
        .checked_sub(2)
        .and_then(|index| components.get(index))
        .copied()
        .unwrap_or_default();
    let grandparent = components
        .len()
        .checked_sub(3)
        .and_then(|index| components.get(index))
        .copied()
        .unwrap_or_default();
    match (parent, last) {
        ("src", "main.rs") => true,
        ("bin" | "examples", file) => std::path::Path::new(file)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs")),
        (_, "main.rs") => matches!(grandparent, "bin" | "examples"),
        _ => false
    }
}

/// Classifies a `main` function, refusing anything the build decides.
fn entry_kind(function: &syn::ItemFn) -> Option<EntryKind> {
    if function
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg"))
    {
        return Some(EntryKind::Conditional);
    }
    if function.sig.asyncness.is_some() {
        return runtime_attribute(&function.attrs).then_some(EntryKind::Runtime);
    }
    Some(EntryKind::Plain)
}

/// Returns whether a runtime attribute generates the real entry.
fn runtime_attribute(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let segments: Vec<String> = attribute
            .path()
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let [.., crate_name, item] = segments.as_slice() else {
            return false;
        };
        RUNTIME_ENTRY_ATTRS
            .iter()
            .any(|(known_crate, known_item)| known_crate == crate_name && known_item == item)
    })
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

    use super::{EntryKind, entry_points};
    use crate::{semantic::behavior::behavior, source::Source};

    fn entries(path: &str, text: &str) -> Vec<(EntryKind, String)> {
        let sources = [Source::new(PathBuf::from(path), text.to_owned())];
        let facts = behavior(&sources);
        entry_points(&sources, &facts)
            .into_iter()
            .map(|entry| (entry.kind, entry.file))
            .collect()
    }

    #[test]
    fn a_binary_root_declares_the_entry_and_a_library_does_not() {
        assert_eq!(
            entries("src/main.rs", "fn main() { println!(\"hi\"); }")
                .first()
                .map(|(kind, _)| *kind),
            Some(EntryKind::Plain)
        );
        assert!(entries("src/lib.rs", "fn main() {}").is_empty());
        assert_eq!(
            entries("src/bin/worker.rs", "fn main() {}").len(),
            1,
            "every file under the binary directory is its own program"
        );
    }

    #[test]
    fn a_main_inside_a_module_is_not_the_process_entry() {
        assert!(entries("src/main.rs", "mod inner { pub fn main() {} }").is_empty());
    }

    #[test]
    fn an_async_main_is_an_entry_only_with_a_runtime_attribute() {
        assert_eq!(
            entries("src/main.rs", "#[tokio::main]\nasync fn main() {}")
                .first()
                .map(|(kind, _)| *kind),
            Some(EntryKind::Runtime)
        );
        assert!(entries("src/main.rs", "async fn main() {}").is_empty());
    }

    #[test]
    fn a_gated_main_is_recorded_as_conditional_rather_than_assumed() {
        assert_eq!(
            entries("src/main.rs", "#[cfg(feature = \"cli\")]\nfn main() {}")
                .first()
                .map(|(kind, _)| *kind),
            Some(EntryKind::Conditional)
        );
    }
}
