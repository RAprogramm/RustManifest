// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where the stream's lock was taken relative to the write site.
//!
//! Both standard streams are guarded by a lock the library takes for the
//! duration of one write. A print macro, and any write whose destination
//! expression builds the handle on the spot, takes that lock at the site.
//! A write through a guard bound earlier does not: the lock was taken
//! once, before the site, and is held across every write through that
//! binding.
//!
//! A form whose cost is one lock acquisition per write is therefore a
//! claim about sites that take the lock themselves, and says nothing
//! about writes through a guard already held.

/// Where the stream's lock was taken relative to the write site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Acquisition {
    /// The site builds the handle, so it takes the lock itself.
    AtSite,
    /// The site writes through a guard bound before it, so the lock was
    /// already held when the site was reached.
    Held,
    /// The destination is not a process stream, or is not identified, so
    /// nothing is claimed about a lock.
    Unknown
}

impl Acquisition {
    /// The phrase this acquisition is named by in a rendered claim.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AtSite => "lock taken at the site",
            Self::Held => "lock already held",
            Self::Unknown => "lock unknown"
        }
    }
}

impl std::fmt::Display for Acquisition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}
