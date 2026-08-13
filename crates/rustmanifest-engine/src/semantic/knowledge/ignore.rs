// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge pack for the `ignore` crate: directory walking that honours
//! the ignore files a repository writes.
//!
//! The walk is the crate's whole point, so what one step of it carries
//! is the row that matters: a built walk hands out directory entries,
//! and every path read off an entry is a path like any other. The
//! failure each step may carry is unwrapped here, the way the return
//! rows unwrap theirs — a written loop settles it on the spot.

use super::{super::behavior::Effect, CrateKnowledge};

/// What the engine knows about `ignore`.
pub(super) const PACK: CrateKnowledge = CrateKnowledge {
    name:           "ignore",
    method_effects: &[
        ("WalkBuilder", "build", Effect::FileSystem),
        ("DirEntry", "metadata", Effect::FileSystem)
    ],
    crate_effect:   Some(Effect::FileSystem)
};
