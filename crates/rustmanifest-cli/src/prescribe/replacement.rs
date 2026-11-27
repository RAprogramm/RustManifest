// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The code a claim licenses in place of the write it holds, and the fact
//! that stands in the way when no replacement can be proposed.

use rustmanifest_engine::{Grade, Multiplicity, Newline, OutputClaim, Stream};

/// The replacement the canonical form asks for, built from the recorded
/// facts alone: the proven text, its terminator, the stream, and whether
/// the owning function can carry a failure out.
pub(super) fn direct_write(claim: &OutputClaim) -> Option<String> {
    let text = claim.write.payload.fully_known()?;
    if claim.execution != Grade::Certain || claim.write.multiplicity != Multiplicity::Once {
        return None;
    }
    let handle = match claim.write.stream {
        Stream::Stdout => "stdout",
        Stream::Stderr => "stderr",
        Stream::CallerSupplied | Stream::Unknown => return None,
    };
    let terminator = match claim.write.newline {
        Newline::Appended => "\\n",
        Newline::None => "",
    };
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!(
        "use std::io::{{Write, {handle}}};\n\nfn {}() -> std::io::Result<()> {{\n    \
         {handle}().lock().write_all(b\"{escaped}{terminator}\")\n}}",
        claim.function
    ))
}

/// Which fact the engine lacks for a prescription it did not make.
pub(super) fn missing(claim: &OutputClaim) -> String {
    if claim.write.payload.fully_known().is_none() {
        return "the written text is not proven in full".to_owned();
    }
    if claim.execution != Grade::Certain {
        return "the write does not run on every path, so replacing it changes behaviour"
            .to_owned();
    }
    if claim.write.multiplicity != Multiplicity::Once {
        return "the write repeats, and a repeated write asks for a buffered writer rather than \
                a direct one"
            .to_owned();
    }
    "the stream this write reaches is not identified".to_owned()
}
