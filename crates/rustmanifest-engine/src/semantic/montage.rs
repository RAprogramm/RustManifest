// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The montage: a plan of file operations applied in memory, so a
//! generated block lands the way an editor would land it — positioned
//! edits and created files — and the result can be read back before
//! any byte touches a disk.
//!
//! The applier owes each byte to at most one edit and applies from the
//! end, so earlier offsets stay valid; a plan whose edits overlap is
//! the planner's defect and answers with nothing rather than a
//! corrupted text. Creation lands after editing, in path order, so a
//! created module file exists by the time the edited parent names it.

use std::path::PathBuf;

use rustmanifest_schema::Range;

use crate::source::Source;

/// One plan of file operations: texts to create and edits to apply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Montage {
    /// Files to create, each with its whole text.
    pub created: Vec<(PathBuf, String)>,
    /// Positioned edits per existing source index.
    pub edited:  Vec<(usize, Vec<(Range, String)>)>
}

/// Applies `montage` to `sources` in memory, or answers with nothing
/// where an edit set overlaps or names a missing source.
#[must_use]
pub fn applied(sources: &[Source], montage: &Montage) -> Option<Vec<Source>> {
    let mut landed: Vec<Source> = sources
        .iter()
        .map(|held| Source::new(held.path().to_path_buf(), held.text().to_owned()))
        .collect();
    for (file, edits) in &montage.edited {
        let source = landed.get(*file)?;
        let mut ordered = edits.clone();
        ordered.sort_by_key(|(range, _)| (range.start, range.end));
        if ordered
            .windows(2)
            .any(|pair| pair[1].0.start < pair[0].0.end)
        {
            return None;
        }
        let mut text = source.text().to_owned();
        for (range, replacement) in ordered.iter().rev() {
            let span = range.start as usize..range.end as usize;
            if span.end > text.len() || !text.is_char_boundary(span.start) {
                return None;
            }
            text.replace_range(span, replacement);
        }
        landed[*file] = Source::new(source.path().to_path_buf(), text);
    }
    for (path, text) in &montage.created {
        landed.push(Source::new(path.clone(), text.clone()));
    }
    Some(landed)
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

    use rustmanifest_schema::Range;

    use super::{Montage, applied};
    use crate::source::Source;

    fn one(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn edits_land_from_the_end_and_creation_follows() {
        let landed = applied(
            &one("fn keep() {}\n"),
            &Montage {
                created: vec![(PathBuf::from("src/next.rs"), "fn next() {}\n".to_owned())],
                edited:  vec![(
                    0,
                    vec![
                        (
                            Range {
                                start: 0, end: 0
                            },
                            "mod next;\n".to_owned()
                        ),
                        (
                            Range {
                                start: 12,
                                end:   13
                            },
                            "\n\nfn more() {}\n".to_owned()
                        ),
                    ]
                )]
            }
        )
        .unwrap();
        assert_eq!(
            landed[0].text(),
            "mod next;\nfn keep() {}\n\nfn more() {}\n"
        );
        assert_eq!(landed[1].path(), PathBuf::from("src/next.rs").as_path());
    }

    #[test]
    fn an_overlapping_plan_is_the_planners_defect_and_lands_nothing() {
        let refused = applied(
            &one("fn keep() {}\n"),
            &Montage {
                created: Vec::new(),
                edited:  vec![(
                    0,
                    vec![
                        (
                            Range {
                                start: 0, end: 5
                            },
                            String::new()
                        ),
                        (
                            Range {
                                start: 3, end: 8
                            },
                            String::new()
                        ),
                    ]
                )]
            }
        );
        assert!(refused.is_none());
    }
}
