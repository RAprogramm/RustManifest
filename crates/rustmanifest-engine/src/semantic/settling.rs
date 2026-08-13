// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Writing down what a compiler settled about a written fixture.
//!
//! Every reading above answers from a resolution and from nothing else, so
//! a test that states an expectation has to state the compiler's answer
//! first. Spelling that out by hand — a location, a byte range, a rendered
//! type — buries the expectation under bookkeeping; this builder names the
//! written fragment instead and works the range out from the text.
//!
//! The fragment is found by its first occurrence, so a fixture wanting a
//! later one says where to look from with [`Settling::after`].

mod declared;
mod identity;
mod standard;

use std::path::PathBuf;

pub use self::{
    declared::{declared_over, settle_declared},
    identity::{identity_of, identity_over, settle_identity},
    standard::{settle_standard_macros, standard_macros_over}
};
use rustmanifest_schema::{
    Location, Range, ResolvedCall, ResolvedField, ResolvedFieldDecl, ResolvedGrant, ResolvedType,
    ResolvedTypeDecl, Resolution
};

/// Everything a compiler settles about a fixture that no reader can
/// derive for itself: the standard macros it writes, the types it
/// declares, and the abilities the library grants those types.
///
/// One call, because a fixture asking about behaviour needs all three
/// and stating them apart buried the subject under bookkeeping.
#[must_use]
pub fn compiler_over(sources: &[crate::source::Source]) -> Resolution {
    let mut resolution = Resolution {
        resolver: "test".to_owned(),
        ..Resolution::default()
    };
    settle_declared(&mut resolution, sources);
    settle_standard_macros(&mut resolution, sources);
    let facts = crate::semantic::behavior_with(sources, Some(&resolution));
    settle_identity(&mut resolution, sources, &facts);
    resolution
}

/// A resolution being written down over one or more fixture files.
pub struct Settling<'text> {
    /// File the fragments named next are written in.
    file:   PathBuf,
    /// Text of that file.
    text:   &'text str,
    /// Offset the next fragment is searched from.
    from:   usize,
    /// What has been settled so far.
    settled: Resolution
}

impl<'text> Settling<'text> {
    /// Starts a resolution over `text`, recorded as the file at `file`.
    #[must_use]
    pub fn over(file: &str, text: &'text str) -> Self {
        Self {
            file:    PathBuf::from(file),
            text,
            from:    0,
            settled: Resolution {
                resolver: "test".to_owned(),
                ..Resolution::default()
            }
        }
    }

    /// Continues over a second file, from its start.
    #[must_use]
    pub fn and_over(mut self, file: &str, text: &'text str) -> Self {
        self.file = PathBuf::from(file);
        self.text = text;
        self.from = 0;
        self
    }

    /// Searches the fragments named next from the start of the file
    /// again.
    #[must_use]
    pub const fn rewound(mut self) -> Self {
        self.from = 0;
        self
    }

    /// Searches the fragments named next from after `written`.
    #[must_use]
    pub fn after(mut self, written: &str) -> Self {
        self.from = self.located(written).range.end as usize;
        self
    }

    /// States the type the compiler settled for the written `fragment`.
    #[must_use]
    pub fn typed(mut self, fragment: &str, rendered: &str) -> Self {
        let at = self.located(fragment);
        self.settled.types.push(ResolvedType {
            at,
            rendered: rendered.to_owned(),
            declared: None
        });
        self
    }

    /// States the type holding the field written at `fragment` and the
    /// type that field itself has.
    #[must_use]
    pub fn field(
        mut self,
        fragment: &str,
        owner: &str,
        produced: &str
    ) -> Self {
        let at = self.located(fragment);
        self.settled.fields.push(ResolvedField {
            at,
            owner: owner.to_owned(),
            produced: produced.to_owned()
        });
        self
    }

    /// States the same about every occurrence of `fragment` from the
    /// cursor on, and says nothing where the fixture writes none.
    ///
    /// A field of one name is read in as many bodies as a fixture has;
    /// naming each occurrence apart would say the same thing four times.
    #[must_use]
    pub fn fields(
        mut self,
        fragment: &str,
        owner: &str,
        produced: &str
    ) -> Self {
        while let Some(at) = self.next_at(fragment) {
            self.from = at.range.end as usize;
            self.settled.fields.push(ResolvedField {
                at,
                owner: owner.to_owned(),
                produced: produced.to_owned()
            });
        }
        self
    }

    /// States the type of every occurrence of `fragment` from the cursor
    /// on, and says nothing where the fixture writes none.
    #[must_use]
    pub fn all_typed(mut self, fragment: &str, rendered: &str) -> Self {
        while let Some(at) = self.next_at(fragment) {
            self.from = at.range.end as usize;
            self.settled.types.push(ResolvedType {
                at,
                rendered: rendered.to_owned(),
                declared: None
            });
        }
        self
    }

    /// Where the next occurrence of `fragment` stands, or nothing where
    /// the rest of the file writes none.
    fn next_at(&self, fragment: &str) -> Option<Location> {
        let start = self.text.get(self.from..)?.find(fragment)? + self.from;
        Some(Location {
            file:  self.file.clone(),
            range: Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(start + fragment.len()).unwrap_or(u32::MAX)
            }
        })
    }

    /// States the call the compiler settled at `fragment`: the function
    /// it reaches, the type it hands back, and the type of the receiver
    /// where it has one.
    #[must_use]
    pub fn call(
        mut self,
        fragment: &str,
        callee: &str,
        returns: &str,
        receiver: Option<&str>
    ) -> Self {
        let at = self.located(fragment);
        self.settled.calls.push(ResolvedCall {
            at,
            callee: callee.to_owned(),
            returns: returns.to_owned(),
            receiver: receiver.map(ToOwned::to_owned),
            receiver_path: None,
            caller_decides: false,
            reaches: None,
            takes: None
        });
        self
    }

    /// States the body the call written at `fragment` reaches, and
    /// nothing else about it.
    ///
    /// Which body a call reaches is the one answer the written name
    /// cannot give: a call into a dependency and a call into this
    /// workspace are spelled the same, and only the reached path tells
    /// them apart.
    #[must_use]
    pub fn reaching(mut self, fragment: &str, reaches: &str) -> Self {
        let at = self.located(fragment);
        self.settled.calls.push(ResolvedCall {
            at,
            callee: reaches.rsplit("::").next().unwrap_or(reaches).to_owned(),
            returns: String::new(),
            receiver: None,
            receiver_path: None,
            caller_decides: false,
            reaches: Some(reaches.to_owned()),
            takes: None
        });
        self
    }

    /// States that the compiler grants `named` the trait `carried`.
    ///
    /// A canonical body rests on what a field's own type carries, and
    /// that is the compiler's answer even for a library type — so a
    /// fixture standing in for a resolution has to be able to say it.
    #[must_use]
    pub fn granting(mut self, named: &str, carried: &str) -> Self {
        self.settled.grants.push(ResolvedGrant {
            at:         Location {
                file:  self.file.clone(),
                range: Range {
                    start: 0, end: 0
                }
            },
            named:      named.to_owned(),
            trait_name: carried.to_owned()
        });
        self
    }

    /// States the fields the compiler read off the declaration of the
    /// type whose name is written at `fragment`.
    ///
    /// What a field holds is the compiler's answer and nothing else, so
    /// a fixture standing in for a resolution states it here or the
    /// graph holds a type with unnamed fields.
    #[must_use]
    pub fn declaring(mut self, fragment: &str, fields: &[(&str, &str)]) -> Self {
        let at = self.located(fragment);
        self.settled.declared.push(ResolvedTypeDecl {
            named: fragment.to_owned(),
            at,
            fields: fields
                .iter()
                .map(|(name, named)| ResolvedFieldDecl {
                    name:  (*name).to_owned(),
                    named: (*named).to_owned()
                })
                .collect()
        });
        self
    }

    /// Hands back the resolution, in the order every lookup bisects.
    #[must_use]
    pub fn done(mut self) -> Resolution {
        self.settled.settle();
        self.settled
    }

    /// Where a written fragment stands in the file being settled.
    fn located(&self, fragment: &str) -> Location {
        let found = self
            .text
            .get(self.from..)
            .and_then(|rest| rest.find(fragment).map(|at| self.from + at));
        assert!(
            found.is_some(),
            "the fragment {fragment:?} is written in the fixture from here on"
        );
        let start = found.unwrap_or_default();
        Location {
            file:  self.file.clone(),
            range: Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(start + fragment.len()).unwrap_or(u32::MAX)
            }
        }
    }
}
