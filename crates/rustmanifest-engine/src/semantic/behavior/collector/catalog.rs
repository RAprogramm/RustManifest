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

/// The library's own wrappers around a value: constructing one produces
/// the value it was handed, in a shell that says which half of an outcome
/// it is.
///
/// The value such a construction stands for is the value written inside
/// it, so naming the construction would put a step between a value and
/// itself.
pub(super) const VALUE_WRAPPERS: [&str; 3] = ["Err", "Ok", "Some"];

/// Adapters changing what a failure carries without changing which value
/// a success is.
///
/// Deliberately narrow. Seeing through one is sound because the value
/// that comes out on success is the value that went in; a method that
/// changes that value is not here, and seeing through it would attribute
/// a value to a site that produced a different one.
pub(super) const OUTCOME_ADAPTERS: [&str; 3] = ["map_err", "ok_or", "ok_or_else"];

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

/// Macros whose every argument is an expression, so the fact channel can
/// read the same tokens the reachability fold reads.
///
/// Deliberately narrower than [`SETTLED_MACROS`]: `matches!` takes a
/// pattern and `stringify!` takes arbitrary tokens, and reading either
/// as an expression would invent call and read facts the source does not
/// carry.
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

/// Iterator and `Option` adapters whose single-parameter closure
/// receives one element of the adapted collection or option.
pub(super) const ITERATOR_ADAPTERS: [&str; 18] = [
    "all",
    "and_then",
    "any",
    "filter",
    "filter_map",
    "find",
    "find_map",
    "flat_map",
    "for_each",
    "inspect",
    "is_some_and",
    "map",
    "map_or",
    "map_or_else",
    "position",
    "retain",
    "skip_while",
    "take_while"
];

/// Iterator constructors and element accessors that keep the
/// collection's element type.
pub(super) const ITERATOR_SOURCES: [&str; 7] = [
    "drain",
    "first",
    "get",
    "into_iter",
    "iter",
    "iter_mut",
    "last"
];

/// Macros that end the current path rather than returning to it.
pub(super) const DIVERGING_MACROS: [&str; 4] = ["panic", "todo", "unimplemented", "unreachable"];

/// Macros whose expansion is known to return to the point that invoked
/// it. A macro outside this list may expand to anything, a divergence
/// included, so its completion is recorded as unproven rather than
/// assumed harmless — the same direction the rest of this channel takes
/// when it cannot see through a construct.
///
/// The list holds standard macros only; a user macro of the same name
/// shadowing one of them would break the reading, which is the standing
/// assumption of every name-keyed recognizer in this tier.
pub(super) const SETTLED_MACROS: [&str; 28] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "cfg",
    "column",
    "concat",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "env",
    "eprint",
    "eprintln",
    "file",
    "format",
    "format_args",
    "include_bytes",
    "include_str",
    "line",
    "matches",
    "module_path",
    "option_env",
    "print",
    "println",
    "stringify",
    "vec",
    "write",
    "writeln"
];
