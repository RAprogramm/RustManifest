// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The channel a write reaches, named as far as the analysis can prove
//! it and recorded as unidentified rather than omitted when it cannot.

use std::fmt;

/// Stream a write goes to, as far as the analysis can prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stream {
    /// The process standard output stream.
    Stdout,
    /// The process standard error stream.
    Stderr,
    /// A writer the caller supplies — a generic parameter or trait
    /// object bound to the write trait, whose destination is decided
    /// outside this function.
    CallerSupplied,
    /// A write whose destination the analysis cannot identify.
    Unknown
}

impl fmt::Display for Stream {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::CallerSupplied => "caller-supplied",
            Self::Unknown => "unknown"
        };
        out.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::Stream;

    #[test]
    fn streams_name_the_channel_they_stand_for() {
        assert_eq!(Stream::Stdout.to_string(), "stdout");
        assert_eq!(Stream::Stderr.to_string(), "stderr");
        assert_eq!(Stream::CallerSupplied.to_string(), "caller-supplied");
        assert_eq!(Stream::Unknown.to_string(), "unknown");
    }
}
