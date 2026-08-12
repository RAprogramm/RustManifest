// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge pack for the `tempfile` crate: temporary files and
//! directories on the filesystem.

use super::{super::behavior::Effect, CrateKnowledge};

/// What the engine knows about `tempfile`.
pub(super) const PACK: CrateKnowledge = CrateKnowledge {
    name:           "tempfile",
    constructors:   &[("tempdir", "TempDir"), ("tempfile", "File")],
    method_returns: &[
        ("TempDir", "path", "Path"),
        ("TempDir", "keep", "PathBuf"),
        ("NamedTempFile", "path", "Path")
    ],
    iterated:       &[],
    method_effects: &[
        ("NamedTempFile", "keep", Effect::FileSystem),
        ("NamedTempFile", "persist", Effect::FileSystem),
        ("TempDir", "close", Effect::FileSystem)
    ],
    crate_effect:   Some(Effect::FileSystem)
};
