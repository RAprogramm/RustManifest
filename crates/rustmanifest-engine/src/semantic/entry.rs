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

mod completion;
mod root;

pub use self::completion::Completion;
use self::{completion::completion_of, root::binary_root};
use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Runtime attributes that generate a real entry around an `async main`.
///
/// The match is on the last two path segments, so a qualifier in front
/// of the crate name still resolves (`::tokio::main`,
/// `crate::rt::tokio::main`). A renamed import is the one case it cannot
/// resolve — a rename replaces the very segment being compared — and an
/// `async main` behind one is refused rather than assumed.
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
    pub fact:       usize,
    /// How it becomes the entry.
    pub kind:       EntryKind,
    /// Path of the binary root file that declares it.
    pub file:       String,
    /// Whether the entry declares a return type, so a failure inside it
    /// can leave as a value instead of ending the process by panic.
    pub returns:    bool,
    /// What the signature decides about the status the process leaves.
    pub completion: Completion
}

/// Finds every process entry point among `sources`.
///
/// A file is a binary root when it is the crate's `src/main.rs`, a file
/// directly under `src/bin`, or the `main.rs` of a directory under
/// `src/bin` or under an `examples` directory outside `src`. Only
/// top-level functions of such a file are considered, so a `main` inside
/// a module is not an entry.
///
/// Each entry is bound to its own fact by the byte range of its own
/// identifier, not by name: a file may declare several `main` items —
/// the `#[cfg(unix)]` / `#[cfg(windows)]` pair is the ordinary case —
/// and each of them describes a different program. An item whose fact
/// is not recorded is skipped rather than bound to a namesake.
#[must_use]
pub fn entry_points(sources: &[Source], facts: &[FunctionFacts]) -> Vec<EntryPoint> {
    let mut entries = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let path = source.path().to_string_lossy().replace('\\', "/");
        if !binary_root(&path) {
            continue;
        }
        let Some(file) = source.tree() else {
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
            let Some(fact) = declaring_fact(facts, index, function) else {
                continue;
            };
            entries.push(EntryPoint {
                fact,
                kind,
                file: path.clone(),
                returns: !matches!(function.sig.output, syn::ReturnType::Default),
                completion: completion_of(&function.sig.output)
            });
        }
    }
    entries
}

/// Index of the fact recorded for exactly this `main` item.
///
/// The fact's range is the byte range of the declaring identifier, so it
/// identifies the item itself; every other `main` in the same file, at
/// any nesting depth, carries a different range.
fn declaring_fact(
    facts: &[FunctionFacts],
    source: usize,
    function: &syn::ItemFn
) -> Option<usize> {
    let span = function.sig.ident.span().byte_range();
    let start = u32::try_from(span.start).unwrap_or(u32::MAX);
    let end = u32::try_from(span.end).unwrap_or(u32::MAX);
    facts.iter().position(|fact| {
        fact.source == source
            && fact.function == "main"
            && fact.range.start == start
            && fact.range.end == end
    })
}

/// Classifies a `main` function, refusing anything the build decides.
///
/// Asyncness is settled first: an `async main` is an entry only when a
/// runtime attribute generates the real one around it, and a
/// configuration gate cannot supply that generation. Only a `main` that
/// would be an entry at all can then be recorded as a conditional one.
fn entry_kind(function: &syn::ItemFn) -> Option<EntryKind> {
    if function.sig.asyncness.is_some() && !runtime_attribute(&function.attrs) {
        return None;
    }
    if function
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg"))
    {
        return Some(EntryKind::Conditional);
    }
    if function.sig.asyncness.is_some() {
        return Some(EntryKind::Runtime);
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
    use crate::{
        semantic::behavior::FunctionFacts,
        source::Source
    };

    fn entries(path: &str, text: &str) -> Vec<(EntryKind, String)> {
        let sources = [Source::new(PathBuf::from(path), text.to_owned())];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        entry_points(&sources, &facts)
            .into_iter()
            .map(|entry| (entry.kind, entry.file))
            .collect()
    }

    /// Every entry of one file as the text its own fact writes paired
    /// with whether its own signature returns, so a fact bound to the
    /// wrong item is visible.
    fn bindings(path: &str, text: &str) -> Vec<(Option<String>, bool)> {
        let sources = [Source::new(PathBuf::from(path), text.to_owned())];
        let facts: Vec<FunctionFacts> = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        entry_points(&sources, &facts)
            .into_iter()
            .map(|entry| {
                let written = facts.get(entry.fact).and_then(|fact| {
                    fact.output_writes
                        .first()
                        .and_then(|write| write.payload.fully_known())
                        .map(ToOwned::to_owned)
                });
                (written, entry.returns)
            })
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

    #[test]
    fn a_main_nested_in_a_module_never_supplies_the_entry_fact() {
        assert_eq!(
            bindings(
                "src/main.rs",
                "mod inner {\n    pub fn main() { println!(\"inner\"); }\n}\nfn main() { \
                 println!(\"outer\"); }"
            ),
            vec![(Some("outer".to_owned()), false)]
        );
    }

    #[test]
    fn each_gated_main_binds_to_its_own_fact_and_its_own_signature() {
        assert_eq!(
            bindings(
                "src/main.rs",
                "#[cfg(unix)]\nfn main() { println!(\"unix\"); }\n#[cfg(windows)]\nfn main() -> \
                 std::io::Result<()> {\n    println!(\"windows\");\n    Ok(())\n}"
            ),
            vec![
                (Some("unix".to_owned()), false),
                (Some("windows".to_owned()), true)
            ]
        );
    }

    #[test]
    fn a_module_directory_named_bin_or_examples_is_not_a_binary_root() {
        assert!(
            entries("src/render/bin/mod.rs", "fn main() {}").is_empty(),
            "`bin` counts only directly under `src`"
        );
        assert!(
            entries("src/examples/demo.rs", "fn main() {}").is_empty(),
            "cargo builds `examples` beside `src`, not inside it"
        );
        assert_eq!(entries("examples/demo.rs", "fn main() {}").len(), 1);
        assert_eq!(entries("src/bin/tool/main.rs", "fn main() {}").len(), 1);
    }

    #[test]
    fn a_gated_async_main_without_a_runtime_is_refused_like_an_ungated_one() {
        assert!(
            entries("src/main.rs", "#[cfg(unix)]\nasync fn main() {}").is_empty(),
            "a configuration gate cannot turn an unwrapped `async main` into an entry"
        );
        assert_eq!(
            entries(
                "src/main.rs",
                "#[cfg(unix)]\n#[tokio::main]\nasync fn main() {}"
            )
            .first()
            .map(|(kind, _)| *kind),
            Some(EntryKind::Conditional)
        );
    }

    #[test]
    fn an_extra_qualifier_resolves_and_a_renamed_runtime_import_does_not() {
        assert_eq!(
            entries("src/main.rs", "#[::tokio::main]\nasync fn main() {}").len(),
            1,
            "the last two segments still name the runtime"
        );
        assert!(
            entries("src/main.rs", "#[rt::main]\nasync fn main() {}").is_empty(),
            "a rename replaces the crate segment, so the match cannot resolve it"
        );
    }
}
