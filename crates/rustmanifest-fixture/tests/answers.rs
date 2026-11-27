// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The compiler answers about a written snippet, and the answer names
//! the file the snippet was written under.

/// A print and a clone are two answers a table used to guess: which
/// item the macro reaches, and what the copying call takes and hands
/// back.
#[test]
fn the_compiler_answers_about_a_written_snippet() {
    let held = rustmanifest_fixture::settled(&[(
        "src/lib.rs",
        "#[derive(Clone)]\npub struct Config {\n    pub retries: u32\n}\n\npub fn go(config: \
         &Config) -> u32 {\n    let copy = config.clone();\n    println!(\"{}\", copy.retries);\n    \
         copy.retries\n}\n"
    )]);
    assert!(
        held.calls
            .iter()
            .any(|row| row.callee == "clone" && row.receiver.as_deref() == Some("&Config")),
        "{:?}",
        held.calls
    );
    assert!(
        held.paths
            .iter()
            .any(|row| row.names == "std::macros::println"),
        "{:?}",
        held.paths.len()
    );
    assert!(
        held.declared.iter().any(|row| row.named == "Config"),
        "{:?}",
        held.declared
    );
    assert!(
        held.calls.iter().all(|row| row.at.file == *"src/lib.rs"),
        "{:?}",
        held.calls
    );
}
