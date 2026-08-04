// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where a read draws from.

/// Where a read draws from, as far as the analysis can prove.
///
/// Five against the write channel's four: the process offers two outward
/// channels and the world offers three inward ones. A channel the
/// analysis cannot identify is recorded rather than omitted, on the same
/// grounds an unidentified stream is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    /// The words the process was started with.
    CommandLine,
    /// The environment the process was started in.
    Environment,
    /// The process's own input stream.
    StandardInput,
    /// A reader the caller supplies, whose source is decided outside this
    /// function.
    CallerSupplied,
    /// A read whose source the analysis cannot identify.
    Unknown
}

impl std::fmt::Display for Channel {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::CommandLine => "command-line",
            Self::Environment => "environment",
            Self::StandardInput => "stdin",
            Self::CallerSupplied => "caller-supplied",
            Self::Unknown => "unknown"
        })
    }
}
