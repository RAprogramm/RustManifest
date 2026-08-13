// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The tables naming what brings a value in from outside the process.
//!
//! Every source is matched on the whole path a call expands to through
//! the file's imports, for the reason the stream table already states: a
//! bare name would credit any library's own lookup — a configuration
//! crate's, a test double's — to the process's own environment, and every
//! claim about what the process was started with would then be made of
//! something else.
//!
//! The build-time environment macros are deliberately absent. They read
//! the environment of the compilation, not of the run, and cross no
//! boundary this channel records.

use crate::semantic::behavior::input::{Channel, Intake, ReadShape, Terminator};

/// Standard-library calls handing the process a value from outside it:
/// the whole path, the channel it draws from, whether the library
/// validates the value as text, the shape it hands over, and whether the
/// call's first argument names one part of that channel.
pub(in crate::semantic::behavior::collector) const INPUT_SOURCE_PATHS: [(
    &[&str],
    Channel,
    Intake,
    ReadShape,
    bool
); 5] = [
    (
        &["std", "env", "args"],
        Channel::CommandLine,
        Intake::Text,
        ReadShape::Sequence,
        false
    ),
    (
        &["std", "env", "args_os"],
        Channel::CommandLine,
        Intake::Raw,
        ReadShape::Sequence,
        false
    ),
    (
        &["std", "env", "var"],
        Channel::Environment,
        Intake::Text,
        ReadShape::Text,
        true
    ),
    (
        &["std", "env", "var_os"],
        Channel::Environment,
        Intake::Raw,
        ReadShape::Text,
        true
    ),
    (
        &["std", "env", "vars"],
        Channel::Environment,
        Intake::Text,
        ReadShape::Sequence,
        false
    )
];

/// Standard-library constructors handing out a handle to the process's
/// own input stream: the whole path, the channel it leads to, the handle
/// type it returns, and the guard type its locking call hands out.
pub(in crate::semantic::behavior::collector) const INTAKE_HANDLE_PATHS: [(
    &str,
    Channel,
    &str,
    &str
); 1] = [(
    "std::io::stdio::stdin",
    Channel::StandardInput,
    "std::io::stdio::Stdin",
    "std::io::stdio::StdinLock"
)];

/// Written type names that name the process's own input stream.
pub(in crate::semantic::behavior::collector) const INTAKE_TYPES: [(&str, Channel, bool); 2] = [
    ("std::io::stdio::Stdin", Channel::StandardInput, false),
    ("std::io::stdio::StdinLock", Channel::StandardInput, true)
];

/// Read-trait methods drawing from a reader, with whether the library
/// validates what they draw as text, the shape taken, and what each does
/// with the line terminator.
pub(in crate::semantic::behavior::collector) const READ_METHODS: [(
    &str,
    Intake,
    ReadShape,
    Terminator
); 4] = [
    (
        "lines",
        Intake::Text,
        ReadShape::Sequence,
        Terminator::Stripped
    ),
    (
        "read_line",
        Intake::Text,
        ReadShape::Text,
        Terminator::Retained
    ),
    (
        "read_to_end",
        Intake::Raw,
        ReadShape::Text,
        Terminator::None
    ),
    (
        "read_to_string",
        Intake::Text,
        ReadShape::Text,
        Terminator::None
    )
];

/// Adapters handing on what a channel yields without changing which
/// channel it came from, with whether each also keeps the position of
/// every element.
///
/// This table is why collecting the words a process was started with is
/// still a read of the command line: without it the boundary is lost at
/// the first adapter, since the receiver of such a call types to nothing
/// and the site records a method on an unknown receiver.
///
/// An adapter that does not keep positions leaves the channel proven and
/// the part drawn unproven, which is the field-by-field degradation the
/// write channel already practises.
pub(in crate::semantic::behavior::collector) const BOUNDARY_ADAPTERS: [(&str, bool); 7] = [
    ("by_ref", true),
    ("collect", true),
    ("filter", false),
    ("into_iter", true),
    ("iter", true),
    ("map", false),
    ("take", true)
];

/// Adapters singling out one element of a sequence by a written position.
pub(in crate::semantic::behavior::collector) const ELEMENT_ADAPTERS: [&str; 2] = ["get", "nth"];
