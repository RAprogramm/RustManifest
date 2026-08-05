// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the engine states about three ordinary programs.
//!
//! A greeting, a calculator, a create-read-update-delete store: the ladder
//! the meaning tier is measured on. The first rung is covered where the
//! sentence is built; these two are covered here, because what they prove
//! is not one recognizer but the whole chain — a value read from outside
//! the process, carried through a call, computed on, and written out.
//!
//! Every assertion below failed before the work it guards. They are the
//! only record that it holds: a run in a terminal is gone the moment the
//! terminal is.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::path::PathBuf;

use rustmanifest_engine::{Narrative, Source, Workspace, program_meaning};

/// The whole statement the engine makes about the entry point of
/// `sources`, licensed clauses and refused one alike.
fn stated(sources: &[(&str, &str)]) -> String {
    let owned: Vec<Source> = sources
        .iter()
        .map(|(path, body)| Source::new(PathBuf::from(path), (*body).to_owned()))
        .collect();
    program_meaning(&Workspace::new(&owned))
        .first()
        .map(|entry| match &entry.narrative {
            Narrative::Whole(said) => said.clone(),
            Narrative::Parts {
                refused: (clause, gap),
                licensed
            } => format!("[{clause} refused: {gap}] {}", licensed.join(" "))
        })
        .unwrap_or_default()
}

/// An ordinary three-argument calculator: two operands read off the
/// command line through a helper, an operator chosen among five written
/// alternatives, the result printed.
const CALCULATOR: &str = "use std::{env, num::ParseIntError, process::ExitCode};\n\
    fn operand(args: &[String], position: usize) -> Result<i64, ParseIntError> {\n\
    \x20   args[position].parse::<i64>()\n\
    }\n\
    fn main() -> ExitCode {\n\
    \x20   let args: Vec<String> = env::args().collect();\n\
    \x20   let left = match operand(&args, 1) {\n\
    \x20       Ok(value) => value,\n\
    \x20       Err(_) => return ExitCode::from(2),\n\
    \x20   };\n\
    \x20   let right = match operand(&args, 3) {\n\
    \x20       Ok(value) => value,\n\
    \x20       Err(_) => return ExitCode::from(2),\n\
    \x20   };\n\
    \x20   let result = match args[2].as_str() {\n\
    \x20       \"+\" => left + right,\n\
    \x20       \"-\" => left - right,\n\
    \x20       \"*\" => left * right,\n\
    \x20       \"/\" => left / right,\n\
    \x20       _ => return ExitCode::from(2),\n\
    \x20   };\n\
    \x20   println!(\"{result}\");\n\
    \x20   ExitCode::SUCCESS\n\
    }\n";

/// The same store as a tutorial actually writes it: commands off the
/// command line, a fallback where none was given, a file behind it.
const TUTORIAL: &str = "use std::{env, fs, io::{self, Write}, process::ExitCode};\n\
    fn save(path: &str, done: bool) -> io::Result<()> { fs::write(path, \
    \x20   format!(\"{done}\")) }\n\
    fn main() -> ExitCode {\n\
    \x20   let args: Vec<String> = env::args().collect();\n\
    \x20   let command = args.get(1).map(String::as_str).unwrap_or(\"list\");\n\
    \x20   let stdout = io::stdout();\n\
    \x20   let mut handle = stdout.lock();\n\
    \x20   let _ = writeln!(handle, \"{command}\");\n\
    \x20   match save(\"todo.txt\", true) {\n\
    \x20       Ok(()) => ExitCode::SUCCESS,\n\
    \x20       Err(_) => ExitCode::from(2),\n\
    \x20   }\n\
    }\n";

/// A store with the four operations of a tutorial, written to a handle
/// rather than to the stream macro.
const STORE: &str = "use std::{collections::BTreeMap, io::{self, Write}, process::ExitCode};\n\
    struct Task { done: bool }\n\
    struct Store { tasks: BTreeMap<u64, Task> }\n\
    impl Store {\n\
    \x20   fn new() -> Self { Self { tasks: BTreeMap::new() } }\n\
    \x20   fn create(&mut self, id: u64) { self.tasks.insert(id, Task { done: false }); }\n\
    \x20   fn read(&self, id: u64) -> Option<&Task> { self.tasks.get(&id) }\n\
    \x20   fn update(&mut self, id: u64) -> bool {\n\
    \x20       match self.tasks.get_mut(&id) { Some(task) => { task.done = true; true } None => \
                     false }\n\
    \x20   }\n\
    \x20   fn delete(&mut self, id: u64) -> bool { self.tasks.remove(&id).is_some() }\n\
    }\n\
    fn main() -> ExitCode {\n\
    \x20   let mut store = Store::new();\n\
    \x20   store.create(1);\n\
    \x20   store.update(1);\n\
    \x20   let listed = store.read(1).is_some();\n\
    \x20   store.delete(1);\n\
    \x20   let stdout = io::stdout();\n\
    \x20   let mut handle = stdout.lock();\n\
    \x20   let _ = writeln!(handle, \"{listed}\");\n\
    \x20   ExitCode::SUCCESS\n\
    }\n";

/// The command line reaches the arithmetic through a binding that carries
/// a written type and through a helper call. Either link broken, the
/// sentence falls back to naming a value it did not derive.
#[test]
fn the_calculator_traces_its_result_back_to_the_command_line() {
    let said = stated(&[("app/src/main.rs", CALCULATOR)]);
    assert!(
        said.contains("takes whole of its command-line"),
        "the intake is not stated: {said}"
    );
    assert!(
        said.contains(
            "conversion to i64 over the result of selection over whole of the \
                       command-line"
        ),
        "the chain from the command line to the number is broken: {said}"
    );
    assert!(
        said.contains("the result of addition over the result of conversion to i64"),
        "the arithmetic does not name what it operates on: {said}"
    );
    assert!(
        !said.contains("did not derive"),
        "the sentence still refuses somewhere: {said}"
    );
}

/// A `match` over the two halves of a fallible result is exhaustive by the
/// type, and the alternative that returns produces no value at all.
#[test]
fn the_calculator_states_its_choices_are_closed_and_that_one_arm_leaves() {
    let said = stated(&[("app/src/main.rs", CALCULATOR)]);
    assert!(
        said.contains("which name every variant of a standard type"),
        "an exhaustive choice is still called unproven: {said}"
    );
    assert!(
        said.contains("of which one accepts every remaining value"),
        "the catch-all choice is not recognised: {said}"
    );
}

/// The written value reaches the handle-taking macro exactly as it reaches
/// the stream macro, a library predicate keeps the origin of its receiver,
/// and a library access keeps the collection the element came out of. The
/// whole chain is derived: no link is a refusal.
#[test]
fn the_store_traces_the_written_value_to_the_collection_it_came_from() {
    let said = stated(&[("app/src/main.rs", STORE)]);
    assert!(
        said.contains(
            "the result of a check that a value is held over the result of selection over the \
             field `Store.tasks` and the written value 1"
        ),
        "the chain from the write to the collection is broken: {said}"
    );
    assert!(
        !said.contains("did not derive"),
        "the sentence still refuses somewhere: {said}"
    );
}

/// The store touches the outside world only through the write it makes,
/// and the engine must not invent more than that.
#[test]
fn the_store_states_the_status_it_leaves_and_claims_nothing_further() {
    let said = stated(&[("app/src/main.rs", STORE)]);
    assert!(
        said.contains("the entry returns a status"),
        "the exit is not stated: {said}"
    );
    assert!(
        !said.contains("command-line"),
        "an intake nothing performs is claimed: {said}"
    );
}

/// A written position singles one element out; what follows transforms
/// that element and cannot change which one it was. Degrading the
/// selector there cost the whole intake clause its licence.
#[test]
fn the_tutorial_keeps_the_position_it_read_through_later_adapters() {
    let said = stated(&[("app/src/main.rs", TUTORIAL)]);
    assert!(
        said.contains("takes position 1 of its command-line"),
        "the position the source settled is lost: {said}"
    );
}

/// A fallback is a choice the source really makes, and the side it falls
/// back to is written right there.
#[test]
fn the_tutorial_states_both_sides_of_its_fallback() {
    let said = stated(&[("app/src/main.rs", TUTORIAL)]);
    assert!(
        said.contains("the written value \"list\""),
        "the fallback the source writes is not named: {said}"
    );
}
