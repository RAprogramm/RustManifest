// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One module of a workspace, as the compiler's own module tree has it.
//!
//! Which file holds which module is decided by the language's rules for
//! mounting a module, by the `path` attribute, and by conditional
//! compilation. Reading it back off file names — a directory here, a file
//! of the same name there, a `mod` line somewhere else — reconstructs a
//! tree the compiler already built, and gets the exceptions wrong.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One module the compiler's tree carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedModule {
    /// Module path from the crate root, crate name first.
    pub names:      String,
    /// File the module's body is written in.
    pub file:       PathBuf,
    /// Whether this module is a crate root.
    pub crate_root: bool,
    /// Module path of the parent, absent for a crate root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent:     Option<String>
}
