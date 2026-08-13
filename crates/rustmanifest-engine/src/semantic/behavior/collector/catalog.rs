// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The name tables this tier recognizes: the print and writer macros, the
//! process stream handles and type names, the macros whose arguments read
//! as expressions, the iterator adapters and sources, and the macros whose
//! expansion is settled or diverging.
//!
//! The tables naming what brings a value in from outside the process sit
//! in a submodule beside this file.
//!
//! Every recognizer keyed on one of these lists shares the same standing
//! assumption: the name is the standard one, and a user item shadowing it
//! would break the reading.

pub(super) mod intake;

use crate::semantic::behavior::output::{Newline, Stream};

/// Print-family macros with the stream each one writes to and whether it
/// appends a line terminator. Every entry also carries
/// [`Effect::ConsoleIo`], which stays the coarse summary category.
///
/// [`Effect::ConsoleIo`]: crate::semantic::behavior::types::Effect::ConsoleIo
pub(super) const OUTPUT_MACROS: [(&str, Stream, Newline); 5] = [
    ("dbg", Stream::Stderr, Newline::Appended),
    ("eprint", Stream::Stderr, Newline::None),
    ("eprintln", Stream::Stderr, Newline::Appended),
    ("print", Stream::Stdout, Newline::None),
    ("println", Stream::Stdout, Newline::Appended)
];

/// Macros that write to a destination handed to them as their first
/// argument. Where [`OUTPUT_MACROS`] fix the stream by name, these
/// leave it to the destination, which is a different question and is
/// answered by the write-destination reading.
pub(super) const WRITER_MACROS: [(&str, Newline); 2] =
    [("write", Newline::None), ("writeln", Newline::Appended)];

/// The write-trait method that puts a caller-provided byte slice on a
/// writer. It is the one write spelling that carries its payload as a
/// value rather than as a format template, so a payload it cannot read
/// is opaque for a different reason than an unreadable format string.
pub(super) const BYTE_WRITE_METHOD: &str = "write_all";

/// The write-trait method that puts a caller-provided string on a
/// formatter or writer. Like the byte method it carries its payload as a
/// value rather than as a format template, and enters no formatting
/// machinery of its own.
pub(super) const TEXT_WRITE_METHOD: &str = "write_str";

/// Standard-library constructors handing out a handle to one of the
/// process's own streams: the full path of the constructor, the stream it
/// leads to, the handle type it returns, and the guard type the handle's
/// locking call hands out.
///
/// The path is matched whole, against the segments a call expands to
/// through the file's imports. A constructor whose name alone were
/// enough would credit any library's own `stdout` — an asynchronous one,
/// a captured one, a test double — to the process stream, and every cost
/// this engine derives from the standard library's buffering would then
/// be claimed of an object that does not buffer that way.
pub(super) const STREAM_HANDLE_PATHS: [(&[&str], Stream, &str, &str); 2] = [
    (
        &["std", "io", "stderr"],
        Stream::Stderr,
        "Stderr",
        "StderrLock"
    ),
    (
        &["std", "io", "stdout"],
        Stream::Stdout,
        "Stdout",
        "StdoutLock"
    )
];

/// The locking call a stream handle answers with a guard held until the
/// guard is dropped.
pub(super) const STREAM_LOCK_METHOD: &str = "lock";

/// The standard-library call that ends the process, taking the number it
/// hands back.
pub(super) const PROCESS_EXIT_PATH: [&str; 3] = ["std", "process", "exit"];

/// The standard-library type whose values are process statuses.
pub(super) const EXIT_CODE_TYPE: &str = "ExitCode";

/// The exit-status constant standing for a run that succeeded.
pub(super) const EXIT_SUCCESS: &str = "SUCCESS";

/// The exit-status constant standing for a run that failed.
pub(super) const EXIT_FAILURE: &str = "FAILURE";

/// The conversion turning a written number into an exit status.
pub(super) const EXIT_FROM: &str = "from";

/// Suffix of the library type names standing for a held stream guard, so
/// a write through one is known to have taken no lock of its own.
pub(super) const STREAM_GUARD_SUFFIX: &str = "Lock";

/// Written type names that name one of the process's own streams, so a
/// binding declared as one is a write to that stream.
pub(super) const STREAM_TYPES: [(&str, Stream); 4] = [
    ("Stderr", Stream::Stderr),
    ("StderrLock", Stream::Stderr),
    ("Stdout", Stream::Stdout),
    ("StdoutLock", Stream::Stdout)
];



/// Macros that end the current path rather than returning to it.
/// Macros whose every argument is an expression, so the fact channel can
/// read the same tokens the reachability fold reads.
pub(super) const EXPRESSION_MACROS: [&str; 20] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "eprint",
    "eprintln",
    "format",
    "format_args",
    "panic",
    "print",
    "println",
    "todo",
    "unimplemented",
    "unreachable",
    "vec",
    "write",
    "writeln"
];

pub(super) const DIVERGING_MACROS: [&str; 4] = ["panic", "todo", "unimplemented", "unreachable"];

