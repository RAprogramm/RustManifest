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
/// appends a line terminator, each named by the item the invocation
/// reaches rather than by the letters somebody typed. Every entry also
/// carries [`Effect::ConsoleIo`], which stays the coarse summary
/// category.
///
/// [`Effect::ConsoleIo`]: crate::semantic::behavior::types::Effect::ConsoleIo
pub(super) const OUTPUT_MACROS: [(&str, Stream, Newline); 5] = [
    ("std::macros::dbg", Stream::Stderr, Newline::Appended),
    ("std::macros::eprint", Stream::Stderr, Newline::None),
    ("std::macros::eprintln", Stream::Stderr, Newline::Appended),
    ("std::macros::print", Stream::Stdout, Newline::None),
    ("std::macros::println", Stream::Stdout, Newline::Appended)
];

/// Macros that write to a destination handed to them as their first
/// argument. Where [`OUTPUT_MACROS`] fix the stream by name, these
/// leave it to the destination, which is a different question and is
/// answered by the write-destination reading.
pub(super) const WRITER_MACROS: [(&str, Newline); 2] = [
    ("core::macros::write", Newline::None),
    ("core::macros::writeln", Newline::Appended)
];

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
pub(super) const STREAM_HANDLE_PATHS: [(&str, Stream, &str, &str); 2] = [
    (
        "std::io::stdio::stderr",
        Stream::Stderr,
        "std::io::stdio::Stderr",
        "std::io::stdio::StderrLock"
    ),
    (
        "std::io::stdio::stdout",
        Stream::Stdout,
        "std::io::stdio::Stdout",
        "std::io::stdio::StdoutLock"
    )
];

/// The locking call a stream handle answers with a guard held until the
/// guard is dropped.
pub(super) const STREAM_LOCK_METHOD: &str = "lock";

/// The standard-library body that ends the process, taking the number
/// it hands back, named by the item a call reaches.
pub(super) const PROCESS_EXIT_PATH: &str = "std::process::exit";

/// The standard-library type whose values are process statuses.
pub(super) const EXIT_CODE_TYPE: &str = "ExitCode";

/// The exit-status constant standing for a run that succeeded.
pub(super) const EXIT_SUCCESS: &str = "SUCCESS";

/// The exit-status constant standing for a run that failed.
pub(super) const EXIT_FAILURE: &str = "FAILURE";

/// The conversion turning a written number into an exit status.
pub(super) const EXIT_FROM: &str = "from";

/// The process's own stream types, each named by the item that declares
/// it, with the stream it stands for and whether a value of it already
/// holds the lock.
///
/// Four written type names answered this before, and answered for any
/// library's own `Stdout` — an asynchronous one, a captured one, a test
/// double — exactly as loudly.
pub(super) const STREAM_TYPES: [(&str, Stream, bool); 4] = [
    ("std::io::stdio::Stderr", Stream::Stderr, false),
    ("std::io::stdio::StderrLock", Stream::Stderr, true),
    ("std::io::stdio::Stdout", Stream::Stdout, false),
    ("std::io::stdio::StdoutLock", Stream::Stdout, true)
];

/// Macros whose every argument is an expression, so the fact channel can
/// read the same tokens the reachability fold reads — each named by the
/// item the invocation reaches.
pub(super) const EXPRESSION_MACROS: [&str; 20] = [
    "alloc::macros::format",
    "alloc::macros::vec",
    "core::macros::assert_eq",
    "core::macros::assert_ne",
    "core::macros::builtin::assert",
    "core::macros::debug_assert",
    "core::macros::debug_assert_eq",
    "core::macros::debug_assert_ne",
    "core::macros::panic",
    "core::macros::todo",
    "core::macros::unimplemented",
    "core::macros::unreachable",
    "core::macros::write",
    "core::macros::writeln",
    "std::macros::dbg",
    "std::macros::eprint",
    "std::macros::eprintln",
    "std::macros::format_args",
    "std::macros::print",
    "std::macros::println"
];
