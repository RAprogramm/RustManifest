// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One candidate edit offered for judgement, whatever its genre: the
//! file it rewrites, the whole rewritten text, and the module file the
//! edit plants where the genre creates one.

use std::path::PathBuf;

/// A file the candidate creates alongside the rewrite: the extraction
/// genre's module file.
pub(super) struct Planted {
    /// Path of the created file, inside the bed.
    pub(super) file: PathBuf,
    /// Whole text of the created file.
    pub(super) text: String
}

/// One candidate edit: the subject named for the verdict, the target
/// file, the whole rewritten text, and the planted file where the
/// genre creates one.
pub(super) struct Candidate {
    /// The subject the verdict names.
    pub(super) subject:   String,
    /// File the edit rewrites, inside the bed.
    pub(super) file:      PathBuf,
    /// The whole text of the file after the edit.
    pub(super) installed: String,
    /// The file the edit creates, where the genre creates one.
    pub(super) planted:   Option<Planted>
}
