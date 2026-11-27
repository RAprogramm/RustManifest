// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The catalogue itself: one entry per canonical form, and the derived
//! library facts that license an entry for one stream.
//!
//! An entry names the properties it stands on, the shape that satisfies
//! them, and the unit a deviation is paid in. Nothing here decides
//! anything about a body; a check reads an entry and contributes facts.

use crate::semantic::{
    behavior::Stream,
    std_derive::{StdFact, StdOutputKnowledge}
};

/// One canonical form and the cost of deviating from it.
///
/// The library guarantee behind a form is never written here: it is
/// derived from the standard library's own sources and cited by property
/// name, so a form whose property the derivation cannot find for a
/// stream produces no finding about that stream at all.
pub(super) struct Form {
    /// Derived properties that license this form for one stream, in the
    /// order they are consulted. An empty list means the form says
    /// nothing about that stream.
    properties:           fn(Stream) -> &'static [&'static str],
    /// The shape that satisfies the property. Editorial: prescription,
    /// not derivation.
    pub(super) canonical: &'static str,
    /// What one deviation costs, as a countable unit paid per event.
    /// The number is computed from the observed facts; only the unit is
    /// named here.
    pub(super) unit:      &'static str
}

/// Catalogue of output forms, indexed by the check that reads it.
pub(super) const LOCK_RUN: Form = Form {
    properties: lock_properties,
    canonical:  "lock the stream handle once, then write every line through that guard",
    unit:       "lock acquisition and release"
};

/// Form for writes standing inside a loop.
pub(super) const LOOP_WRITE: Form = Form {
    properties: buffering_properties,
    canonical:  "wrap the locked handle in a buffered writer and flush once after the loop",
    unit:       "write syscall"
};

/// Form for constants carried through the formatting machinery.
pub(super) const CONSTANT_TEXT: Form = Form {
    properties: format_properties,
    canonical:  "write the constant's bytes directly to the locked handle",
    unit:       "format-argument construction"
};

/// Form for an entry point that cannot carry a write failure out.
pub(super) const WRITE_ERROR: Form = Form {
    properties: panic_properties,
    canonical:  "return a result from the entry point and hand the write's result back",
    unit:       "panic instead of an exit code"
};

/// Each stream holds its own lock, so each proves its own locking
/// separately.
const fn lock_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout => &["lock-per-call:stdout"],
        Stream::Stderr => &["lock-per-call:stderr"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// A write reaches the operating system on every call when the stream
/// buffers by line — every print ends a line — or does not buffer at
/// all. A stream buffering by block amortizes the writes, so
/// `block-buffered:*` is deliberately absent from these lists and the
/// form stays silent where the derivation reports it.
const fn buffering_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout => &["line-buffered:stdout", "unbuffered:stdout"],
        Stream::Stderr => &["line-buffered:stderr", "unbuffered:stderr"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// Both standard streams reach the operating system through one generic
/// print function, so the properties read out of that function hold for
/// either stream.
const fn format_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout | Stream::Stderr => &["format-machinery"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// Same generic print function, same reasoning as
/// [`format_properties`].
const fn panic_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout | Stream::Stderr => &["panic-on-failure"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// The derived fact licensing one form for one stream, or `None` when
/// the library sources proved nothing that licenses it there.
pub(super) fn support<'a>(
    form: &Form,
    stream: Stream,
    library: &'a StdOutputKnowledge
) -> Option<&'a StdFact> {
    (form.properties)(stream)
        .iter()
        .find_map(|property| library.fact(property))
}

/// Library knowledge for the active toolchain, derived once per process
/// and shared by every check, or nothing when the sources are not
/// installed.
pub(super) fn library_knowledge() -> &'static StdOutputKnowledge {
    static DERIVED: std::sync::OnceLock<StdOutputKnowledge> = std::sync::OnceLock::new();
    DERIVED.get_or_init(|| {
        crate::semantic::std_derive::std_source_root()
            .map_or_default(|root| crate::semantic::std_derive::output_knowledge(&root))
    })
}

/// The streams a form can speak about, in report order.
pub(super) const STANDARD_STREAMS: [Stream; 2] = [Stream::Stdout, Stream::Stderr];

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{
        CONSTANT_TEXT, LOCK_RUN, LOOP_WRITE, STANDARD_STREAMS, WRITE_ERROR, library_knowledge,
        support
    };
    use crate::semantic::behavior::Stream;

    /// Every finding quotes the library at the place its property was
    /// derived from, and no form speaks about a stream the derivation
    /// says nothing about.
    #[test]
    fn every_form_is_backed_by_derived_library_text() {
        let library = library_knowledge();
        for form in [&LOCK_RUN, &LOOP_WRITE, &CONSTANT_TEXT, &WRITE_ERROR] {
            for stream in [Stream::CallerSupplied, Stream::Unknown] {
                assert!(support(form, stream, library).is_none());
            }
            for stream in STANDARD_STREAMS {
                let derived = support(form, stream, library)
                    .expect("every form must be licensed for both standard streams");
                assert_ne!(derived.snippet, "");
                assert!(derived.line > 0);
            }
        }
    }
}
