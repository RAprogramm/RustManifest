// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The closed loop of emission: the engine installs a block it wrote,
//! reads the result back with its own reading, and states what the
//! installation gave.
//!
//! Nothing here trusts the generator. The gain is not copied from the
//! plan — it is derived from the installed text the same way any
//! foreign code would be read: the type graph answers whether the
//! trait is now implemented, and the behavioral facts of the installed
//! functions answer what they carry — including whether a body still
//! panics, which is exactly the difference between a skeleton
//! obligation and a written body. An installation whose reading does
//! not confirm the intent answers with nothing: an unconfirmed insert
//! is not a gain.

use super::{
    behavior::{Effect, behavior},
    calls::{call_graph, resolution_context},
    extract::extract_function,
    graph::TypeGraph,
    implement::impl_skeleton,
    montage::{Montage, applied}
};
use crate::source::Source;

/// What one confirmed installation gave, as the reading derives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gain {
    /// The whole text of the file after installation.
    pub installed:   String,
    /// The trait the type now provably implements.
    pub implemented: String,
    /// Effects the installed functions carry, read from their bodies.
    pub effects:     Vec<Effect>,
    /// Whether an installed body can still end the path by panic — a
    /// skeleton obligation not yet written out.
    pub panics:      bool
}

/// Installs the impl of `trait_name` for `type_name` into source
/// `file` and confirms the gain by reading the result back, or answers
/// with nothing where the plan or the confirmation fails.
#[must_use]
pub fn install_impl(
    sources: &[Source],
    file: usize,
    type_name: &str,
    trait_name: &str
) -> Option<Gain> {
    let types = TypeGraph::from_sources(sources);
    let written = impl_skeleton(sources, &types, type_name, trait_name)?;
    let source = sources.get(file)?;
    let end = u32::try_from(source.text().len()).ok()?;
    let replaced = applied(
        sources,
        &Montage {
            created: Vec::new(),
            edited:  vec![(
                file,
                vec![(
                    rustmanifest_schema::Range {
                        start: end,
                        end
                    },
                    format!("\n{written}\n")
                )]
            )]
        }
    )?;
    let installed = replaced.get(file)?.text().to_owned();
    let reread = TypeGraph::from_sources(&replaced);
    let now_implemented = reread.traits_implemented(type_name).contains(&trait_name);
    if !now_implemented {
        return None;
    }
    let before: std::collections::BTreeSet<String> = behavior(sources)
        .into_iter()
        .map(|fact| fact.function)
        .collect();
    let after = behavior(&replaced);
    let mut effects = Vec::new();
    let mut panics = false;
    for fact in after {
        if before.contains(&fact.function) {
            continue;
        }
        effects.extend(fact.effects.iter().copied());
        panics |= fact.flags.panics();
    }
    effects.sort();
    effects.dedup();
    Some(Gain {
        installed,
        implemented: trait_name.to_owned(),
        effects,
        panics
    })
}

/// What one confirmed relocation gave, as the reading derives it.
#[derive(Debug, Clone)]
pub struct Relocated {
    /// The sources after the montage, the created module included.
    pub sources:  Vec<Source>,
    /// The moved function's name.
    pub function: String,
    /// Where the function now lives.
    pub landed:   std::path::PathBuf
}

/// Extracts the function at `offset` through the montage and confirms
/// the relocation by reading the result back.
///
/// The confirmation is threefold: both texts parse, the function is
/// declared exactly once — in the created file — and the callers left
/// behind still resolve their call to it.
#[must_use]
pub fn install_extraction(sources: &[Source], file: usize, offset: u32) -> Option<Relocated> {
    let plan = extract_function(sources, file, offset)?;
    let landed = applied(
        sources,
        &Montage {
            created: vec![(plan.new_file.clone(), plan.new_text.clone())],
            edited:  vec![(file, plan.edits.clone())]
        }
    )?;
    for source in &landed {
        syn::parse_file(source.text()).ok()?;
    }
    let name = plan.new_file.file_stem()?.to_str()?.to_owned();
    let facts = behavior(&landed);
    let declared: Vec<usize> = facts
        .iter()
        .enumerate()
        .filter(|(_, fact)| fact.function == name)
        .map(|(index, _)| index)
        .collect();
    let [held] = declared[..] else {
        return None;
    };
    let new_index = landed.len() - 1;
    if facts[held].source != new_index {
        return None;
    }
    let graph = call_graph(&facts, &resolution_context(&landed));
    let called_back = facts
        .iter()
        .enumerate()
        .filter(|(_, fact)| fact.source == file)
        .any(|(index, _)| graph.callees(index).contains(&held));
    let mentioned = landed
        .get(file)
        .is_some_and(|source| source.text().contains(&name));
    if mentioned && !called_back {
        return None;
    }
    Some(Relocated {
        sources:  landed,
        function: name,
        landed:   plan.new_file
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

    use super::install_impl;

    fn one(text: &str) -> Vec<crate::source::Source> {
        vec![crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )]
    }

    /// A canon body written from proven fields installs as a working
    /// function: the re-reading confirms the trait and sees no panic.
    #[test]
    fn a_written_body_installs_and_reads_back_clean() {
        let sources = one("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n");
        let gain = install_impl(&sources, 0, "User", "Display").unwrap();
        assert_eq!(gain.implemented, "Display");
        assert!(!gain.panics, "{:?}", gain.installed);
        assert!(gain.installed.contains("write!(out, \"name: {}, age: {}\""));
    }

    /// A skeleton obligation installs but the re-reading tells the
    /// difference: the body still panics, and the gain says so.
    #[test]
    fn a_skeleton_installs_but_the_reading_names_the_debt() {
        let sources = one("pub struct Batch {\n    pub held: Vec<String>,\n}\n");
        let gain = install_impl(&sources, 0, "Batch", "Display").unwrap();
        assert!(gain.panics, "{:?}", gain.installed);
    }

    /// An already implemented trait plans nothing, so nothing installs.
    #[test]
    fn a_refused_plan_installs_nothing() {
        let sources =
            one("#[derive(Default)]\npub struct Disk;\nfn keep() -> Disk { Disk::default() }\n");
        assert!(install_impl(&sources, 0, "Disk", "Default").is_none());
    }

    /// A relocation through the montage reads back whole: both texts
    /// parse, the function lives once in the created file, and the
    /// caller left behind still resolves its call to it.
    #[test]
    fn a_relocation_reads_back_whole() {
        let sources = one(
            "fn keep(count: usize) -> usize { render(count) }\n\nfn render(count: usize) -> \
             usize {\n    count + 1\n}\n"
        );
        let offset = u32::try_from(sources[0].text().find("fn render").unwrap()).unwrap() + 1;
        let moved = super::install_extraction(&sources, 0, offset).unwrap();
        assert_eq!(moved.function, "render");
        assert_eq!(moved.landed, std::path::PathBuf::from("src/render.rs"));
        assert!(moved.sources[0].text().contains("mod render;"));
        assert!(!moved.sources[0].text().contains("fn render("));
    }
}
