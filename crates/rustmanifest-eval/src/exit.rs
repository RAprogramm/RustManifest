// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Process exit codes the harness reports its verdict through.

/// Exit code: counts match the baseline and every budget holds.
pub const EXIT_CLEAN: u8 = 0;
/// Exit code: at least one count drifted or a performance budget broke.
pub const EXIT_REGRESSION: u8 = 1;
/// Exit code: operational failure (IO, parsing, engine error).
pub const EXIT_ERROR: u8 = 2;
