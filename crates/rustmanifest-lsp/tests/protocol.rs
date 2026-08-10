// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The server speaks the protocol end to end: a client opens a document
//! carrying a violation and receives the diagnostic for it, with the
//! rule identifier the reader can ask `explain` about.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use lsp_server::{Connection, Message, Notification, Request};
use lsp_types::notification::Notification as _;

/// Opens one document over a fresh in-memory server and swallows the
/// published diagnostics: the boilerplate every surface test shares.
fn opened(text: &str) -> (Connection, std::thread::JoinHandle<()>) {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server).unwrap());
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": text
                }
            })
        )))
        .unwrap();
    let _diagnostics = client.receiver.recv().unwrap();
    (client, handle)
}

/// Shuts a session opened by [`opened`] down and joins the server.
fn closed(client: Connection, handle: std::thread::JoinHandle<()>) {
    client
        .sender
        .send(Message::Request(Request::new(
            99.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap();
}

/// A source the bundled rules flag: a hardcoded credential.
const FLAGGED: &str = "fn main() {\n    let password = \"hunter2\";\n}\n";

#[test]
fn an_opened_document_with_a_violation_receives_its_diagnostic() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));

    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let Message::Response(initialize_answer) = client.receiver.recv().unwrap() else {
        panic!("the initialize request must be answered first");
    };
    let settled = initialize_answer.response_result.as_ref().unwrap();
    let synchronizes = settled["capabilities"]["textDocumentSync"].is_number();
    assert!(
        synchronizes,
        "a server whose declared capabilities carry no text sync is never sent a document, and \
         everything after this line tests nothing: {initialize_answer:?}"
    );
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();

    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": FLAGGED
                }
            })
        )))
        .unwrap();

    let Message::Notification(published) = client.receiver.recv().unwrap() else {
        panic!("the first message after an open must be the diagnostics");
    };
    assert_eq!(
        published.method,
        lsp_types::notification::PublishDiagnostics::METHOD
    );
    let diagnostics = published.params["diagnostics"].as_array().unwrap().clone();
    assert!(
        !diagnostics.is_empty(),
        "a violation in the opened text must produce a diagnostic"
    );
    assert!(
        diagnostics
            .iter()
            .all(|entry| entry["source"] == "rust-manifest" && entry["code"].is_string()),
        "{diagnostics:?}"
    );

    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// A source whose function clones, allocates, takes a borrowed
/// parameter, and reaches the file system only through a helper — the
/// memory story a card must tell, and the inference it must carry.
const HUNGRY: &str = "fn persist(entry: &str) { std::fs::write(\"log\", entry).ok(); }\nfn \
                      hoard(input: &str) -> Vec<String> {\n    let mut out = Vec::new();\n    \
                      out.push(input.to_owned());\n    out.push(format!(\"{input}\"));\n    \
                      persist(input);\n    out\n}\n";

#[test]
fn a_hover_on_a_function_name_answers_with_its_recorded_facts() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": HUNGRY
                }
            })
        )))
        .unwrap();
    let _diagnostics = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/hover".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "position": { "line": 1, "character": 4 }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a hover request must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    let value = held["contents"]["value"].as_str().unwrap();
    assert!(
        value.contains("```rust\nfn hoard(input: &str)\n```"),
        "{value}"
    );
    assert!(value.contains("clones: 1"), "{value}");
    assert!(value.contains("allocations:"), "{value}");
    assert!(value.contains("taken borrowed"), "{value}");
    assert!(value.contains("reached through calls: `fs`"), "{value}");
    assert!(value.contains("**why `fs`**"), "{value}");
    assert!(
        value.contains("1. fn `hoard` calls fn `persist` — resolved call edge"),
        "the card must carry the inference, step by step: {value}"
    );
    assert!(
        value.contains("proven by the written call `std::fs::write`"),
        "{value}"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// The workspace command answers with the inference as data, so any
/// client scripts it.
#[test]
fn the_why_command_answers_with_the_inference_as_data() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": HUNGRY
                }
            })
        )))
        .unwrap();
    let _diagnostics = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.why",
                "arguments": ["file:///demo/src/lib.rs", "hoard", "fs"]
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("the command must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    assert!(
        held["conclusion"]
            .as_str()
            .unwrap()
            .contains("reaches `fs` through `persist`"),
        "{held}"
    );
    assert_eq!(held["steps"].as_array().unwrap().len(), 2, "{held}");
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// Two documents, the chain crossing between them: the card of the
/// caller in one file carries the inference down to the proof written
/// in the other.
#[test]
fn a_chain_crossing_two_open_documents_answers_whole() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    for (uri, text) in [
        (
            "file:///demo/src/store.rs",
            "pub fn persist(entry: &str) { std::fs::write(\"log\", entry).ok(); }\n"
        ),
        (
            "file:///demo/src/lib.rs",
            "mod store;\nfn keep(entry: &str) { crate::store::persist(entry); }\n"
        )
    ] {
        client
            .sender
            .send(Message::Notification(Notification::new(
                lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
                serde_json::json!({
                    "textDocument": {
                        "uri": uri, "languageId": "rust", "version": 1, "text": text
                    }
                })
            )))
            .unwrap();
        let _diagnostics = client.receiver.recv().unwrap();
    }
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.why",
                "arguments": ["file:///demo/src/lib.rs", "keep", "fs"]
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("the command must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    assert!(
        held["conclusion"]
            .as_str()
            .unwrap()
            .contains("reaches `fs` through `persist`"),
        "a chain crossing two documents must answer whole: {held}"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// The interview scene itself: a function taking a `String` by value,
/// and a caller handing its local over. The hover on the written call
/// must tell the move.
const HANDED: &str = "fn foo(value: String) {}\nfn main() { let s = String::from(\"hello\"); \
                      foo(s); println!(\"{s}\"); }\n";

#[test]
fn a_hover_on_a_written_call_tells_where_ownership_goes() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": HANDED
                }
            })
        )))
        .unwrap();
    let Message::Notification(published) = client.receiver.recv().unwrap() else {
        panic!("the first message after an open must be the diagnostics");
    };
    let diagnostics = published.params["diagnostics"].as_array().unwrap().clone();
    assert!(
        diagnostics
            .iter()
            .any(|entry| entry["code"] == "RM-MEM-004"),
        "the use after the move must be diagnosed where it stands: {diagnostics:?}"
    );
    let line = HANDED.lines().nth(1).unwrap();
    let character = line.find("foo(s)").unwrap() + 1;
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/hover".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "position": { "line": 1, "character": character }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a hover request must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    let value = held["contents"]["value"].as_str().unwrap();
    assert!(value.contains("**ownership at this call**"), "{value}");
    assert!(
        value.contains("`value: String` is taken by value, and `String` is not `Copy`"),
        "{value}"
    );
    assert!(value.contains("unusable after this line"), "{value}");
    assert!(
        value.contains("calling `foo` moves 1 value(s) in"),
        "{value}"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// A type and a trait it does not implement: the implement command must
/// answer with the written impl block, signatures from the declaration.
const UNWRITTEN: &str = "struct Disk;\ntrait Sink {\n    fn write(&mut self, chunk: &[u8]) -> \
                         usize;\n    fn flush(&mut self) {}\n}\n";

#[test]
fn the_implement_command_writes_the_impl_block() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": UNWRITTEN
                }
            })
        )))
        .unwrap();
    let _diagnostics = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.implement",
                "arguments": ["Disk", "Sink"]
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("the command must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    let text = held["text"].as_str().unwrap();
    assert!(text.starts_with("impl Sink for Disk {"), "{text}");
    assert!(
        text.contains("fn write(&mut self, chunk: &[u8]) -> usize {"),
        "{text}"
    );
    assert!(text.contains("todo!()"), "{text}");
    assert!(!text.contains("fn flush"), "{text}");
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// A cursor on the type's written name must offer the implementable
/// traits in the editor's action list, each carrying its edit.
#[test]
fn a_code_action_on_a_type_offers_the_unimplemented_traits() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": UNWRITTEN
                }
            })
        )))
        .unwrap();
    let _diagnostics = client.receiver.recv().unwrap();
    let character = UNWRITTEN.find("Disk").unwrap() + 1;
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/codeAction".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "range": {
                    "start": { "line": 0, "character": character },
                    "end": { "line": 0, "character": character }
                },
                "context": { "diagnostics": [] }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a code action request must be answered");
    };
    let offered = answer.response_result.as_ref().unwrap().as_array().unwrap();
    let titles: Vec<&str> = offered
        .iter()
        .map(|action| action["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"implement Sink for Disk"), "{titles:?}");
    assert!(titles.contains(&"implement Display for Disk"), "{titles:?}");
    let written = offered
        .iter()
        .find(|action| action["title"] == "implement Sink for Disk")
        .unwrap();
    let edits = &written["edit"]["documentChanges"][0];
    assert_eq!(edits["textDocument"]["uri"], "file:///demo/src/lib.rs");
    let text = edits["edits"][0]["newText"].as_str().unwrap();
    assert!(text.contains("impl Sink for Disk {"), "{text}");
    assert!(
        text.contains("fn write(&mut self, chunk: &[u8]) -> usize {"),
        "{text}"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// A type with fields, derives, and a written impl: the hover on its
/// name must card what it holds, implements, and weighs.
const TYPED: &str = "#[derive(Clone, Debug)]\nstruct User {\n    id: u64,\n    name: String,\n}\n\
                     impl std::fmt::Display for User {\n    fn fmt(&self, out: &mut \
                     std::fmt::Formatter<'_>) -> std::fmt::Result {\n        write!(out, \
                     \"{}\", self.name)\n    }\n}\n";

#[test]
fn a_hover_on_a_type_name_cards_its_facts() {
    let (client, handle) = opened(TYPED);
    let line = TYPED
        .lines()
        .position(|held| held.contains("struct User"))
        .unwrap();
    let character = TYPED.lines().nth(line).unwrap().find("User").unwrap() + 1;
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/hover".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "position": { "line": line, "character": character }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a hover request must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    let value = held["contents"]["value"].as_str().unwrap();
    assert!(value.contains("```rust\nstruct User {"), "{value}");
    assert!(value.contains("    name: String,"), "{value}");
    assert!(value.contains("**traits**"), "{value}");
    assert!(
        value.contains("✓ `Clone` — the definition derives it"),
        "{value}"
    );
    assert!(value.contains("✓ `Display` — a written impl"), "{value}");
    assert!(
        value.contains(
            "✗ `Copy` — not derived, and `name: String` owns heap data — a type holding it cannot \
             be `Copy`"
        ),
        "{value}"
    );
    assert!(
        value.contains("– `Default` — not written in the workspace"),
        "{value}"
    );
    assert!(value.contains("are the compiler's answer"), "{value}");
    assert!(value.contains("**memory**"), "{value}");
    assert!(
        value.contains("byte(s) on the stack — that is what every move or copy of it touches"),
        "{value}"
    );
    assert!(
        value.contains("own heap data: the contents live in the heap"),
        "{value}"
    );
    closed(client, handle);
}

/// The inlay hints of the interview program: the move visible at the
/// argument itself, no hover asked.
#[test]
fn inlay_hints_mark_the_fate_of_each_argument() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": HANDED
                }
            })
        )))
        .unwrap();
    let _diagnostics = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/inlayHint".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 2, "character": 0 }
                }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("an inlay hint request must be answered");
    };
    let hints = answer.response_result.as_ref().unwrap().as_array().unwrap();
    assert_eq!(hints.len(), 1, "{hints:?}");
    assert_eq!(hints[0]["label"], "moves", "{hints:?}");
    let line = HANDED.lines().nth(1).unwrap();
    let argument = line.find("foo(s)").unwrap() + 4;
    assert_eq!(hints[0]["position"]["line"], 1, "{hints:?}");
    assert_eq!(hints[0]["position"]["character"], argument, "{hints:?}");
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// A finding that carries a synthesized fix must offer it as a
/// quickfix action, edits included.
const FIXABLE: &str = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                       report.total }\n";

#[test]
fn a_diagnostic_with_a_fix_offers_it_as_a_quickfix() {
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({ "capabilities": {} })
        )))
        .unwrap();
    let _initialize_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "initialized".to_owned(),
            serde_json::json!({})
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///demo/src/lib.rs",
                    "languageId": "rust",
                    "version": 1,
                    "text": FIXABLE
                }
            })
        )))
        .unwrap();
    let Message::Notification(published) = client.receiver.recv().unwrap() else {
        panic!("the first message after an open must be the diagnostics");
    };
    let diagnostics = published.params["diagnostics"].as_array().unwrap().clone();
    assert!(
        diagnostics
            .iter()
            .any(|entry| entry["code"] == "RM-MEM-001"),
        "{diagnostics:?}"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/codeAction".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 10 }
                },
                "context": { "diagnostics": [] }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a code action request must be answered");
    };
    let offered = answer.response_result.as_ref().unwrap().as_array().unwrap();
    let fix = offered
        .iter()
        .find(|action| action["kind"] == "quickfix")
        .unwrap_or_else(|| panic!("a finding with a fix must offer a quickfix: {offered:?}"));
    assert!(
        fix["title"].as_str().unwrap().contains("RM-MEM-001"),
        "{fix:?}"
    );
    let edits = &fix["edit"]["documentChanges"][0]["edits"];
    let line = FIXABLE.lines().nth(1).unwrap();
    let at = line.find("Report").unwrap();
    assert!(
        edits.as_array().unwrap().iter().any(|edit| {
            edit["newText"] == "&"
                && edit["range"]["start"]["line"] == 1
                && edit["range"]["start"]["character"] == at
        }),
        "the quickfix must insert the borrow right before the owned type: {edits:?}"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            3.into(),
            "shutdown".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    let _shutdown_answer = client.receiver.recv().unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null
        )))
        .unwrap();
    drop(client);
    handle.join().unwrap().unwrap();
}

/// A cursor inside a free function offers the extraction: the new
/// module file created and both files edited in one workspace edit.
const CROWDED: &str = "use std::collections::BTreeMap;\n\nfn keep(store: &BTreeMap<u64, \
                       String>) -> usize { store.len() }\n\nfn render(store: &BTreeMap<u64, \
                       String>) -> usize {\n    keep(store)\n}\n";

#[test]
fn a_function_offers_its_extraction_into_its_own_file() {
    let (client, handle) = opened(CROWDED);
    let line = CROWDED
        .lines()
        .position(|held| held.contains("fn render"))
        .unwrap();
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/codeAction".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "range": {
                    "start": { "line": line, "character": 4 },
                    "end": { "line": line, "character": 4 }
                },
                "context": { "diagnostics": [] }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a code action request must be answered");
    };
    let offered = answer.response_result.as_ref().unwrap().as_array().unwrap();
    let extraction = offered
        .iter()
        .find(|action| action["kind"] == "refactor.extract")
        .unwrap_or_else(|| panic!("the extraction must be offered: {offered:?}"));
    assert_eq!(
        extraction["title"], "extract `render` into /demo/src/render.rs",
        "{extraction:?}"
    );
    let changes = extraction["edit"]["documentChanges"].as_array().unwrap();
    assert_eq!(changes[0]["kind"], "create", "{changes:?}");
    assert_eq!(changes[0]["uri"], "file:///demo/src/render.rs");
    let created = changes[1]["edits"][0]["newText"].as_str().unwrap();
    assert!(
        created.contains("use std::collections::BTreeMap;"),
        "{created}"
    );
    assert!(created.contains("use super::{keep};"), "{created}");
    assert!(created.contains("pub(super) fn render"), "{created}");
    let old_edits = changes[2]["edits"].as_array().unwrap();
    assert!(
        old_edits.iter().any(
            |edit| edit["newText"] == "mod render;\nuse self::render::render;\n"
                && edit["range"]["start"]["line"] == 0
                && edit["range"]["start"]["character"] == 0
        ),
        "the mod line must land on top of the old file: {old_edits:?}"
    );
    assert!(
        old_edits.iter().any(|edit| edit["newText"] == ""),
        "the function must leave no residue: {old_edits:?}"
    );
    closed(client, handle);
}

/// A cursor on a type's declaration offers moving the type with its
/// impls into a snake-named module file.
const RETINUE: &str = "fn count(store: &Store) -> usize { store.len() }\n\nstruct Store {\n    \
                       held: Vec<u64>,\n}\n\nimpl Store {\n    fn len(&self) -> usize {\n        \
                       self.held.len()\n    }\n}\n";

#[test]
fn a_type_offers_its_extraction_with_its_impls() {
    let (client, handle) = opened(RETINUE);
    let line = RETINUE
        .lines()
        .position(|held| held.contains("struct Store"))
        .unwrap();
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/codeAction".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "range": {
                    "start": { "line": line, "character": 8 },
                    "end": { "line": line, "character": 8 }
                },
                "context": { "diagnostics": [] }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a code action request must be answered");
    };
    let offered = answer.response_result.as_ref().unwrap().as_array().unwrap();
    let extraction = offered
        .iter()
        .find(|action| action["kind"] == "refactor.extract")
        .unwrap_or_else(|| panic!("the type extraction must be offered: {offered:?}"));
    let changes = extraction["edit"]["documentChanges"].as_array().unwrap();
    assert_eq!(
        changes[0]["uri"], "file:///demo/src/store.rs",
        "{changes:?}"
    );
    let created = changes[1]["edits"][0]["newText"].as_str().unwrap();
    assert!(created.contains("pub(super) struct Store"), "{created}");
    assert!(created.contains("pub(super) fn len"), "{created}");
    closed(client, handle);
}

/// The intents command must answer with what the workspace's own
/// written calls prove is missing, each intent citing its evidence —
/// the program naming its need before anyone asks.
#[test]
fn the_intents_command_names_what_the_written_calls_ask_for() {
    let (client, handle) = opened(
        "pub struct Job {\n    pub name: String,\n}\n\npub fn make() -> Job {\n    \
         Job::default()\n}\n"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.intents",
                "arguments": []
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("the command must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    let intents = held["intents"].as_array().unwrap();
    assert_eq!(intents.len(), 1, "{intents:?}");
    assert_eq!(intents[0]["type"], "Job", "{intents:?}");
    assert_eq!(intents[0]["trait"], "Default", "{intents:?}");
    assert!(
        intents[0]["evidence"]["fact"]
            .as_str()
            .unwrap()
            .contains("Job::default"),
        "{intents:?}"
    );
    closed(client, handle);
}

/// The buffering measurement must reach the cursor: the card of a
/// writing function says where it writes, through what buffering, and
/// under what lock — the same reading the facts export carries.
#[test]
fn a_hover_tells_where_the_function_writes_and_through_what() {
    let (client, handle) = opened("fn shout() {\n    println!(\"hi\");\n}\n");
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/hover".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "position": { "line": 0, "character": 4 }
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("a hover request must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    let value = held["contents"]["value"].as_str().unwrap();
    assert!(value.contains("**output**"), "{value}");
    assert!(value.contains("stdout: 1 write(s)"), "{value}");
    assert!(value.contains("the stream's own buffering"), "{value}");
    assert!(value.contains("lock taken at the site"), "{value}");
    closed(client, handle);
}

/// The deduce command must answer with the chain itself as data: the
/// conclusion, every step a recorded fact with its reason, and the
/// confirmed gain — the "why installed" genre reaching the editor the
/// way "why effect" already does.
#[test]
fn the_deduce_command_carries_the_chain_to_the_client() {
    let (client, handle) =
        opened("pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n");
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.deduce",
                "arguments": ["User", "Display"]
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("the command must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    assert!(
        held["conclusion"]
            .as_str()
            .unwrap()
            .contains("by deduction"),
        "{held}"
    );
    let steps = held["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 6, "{steps:?}");
    assert!(
        steps
            .iter()
            .any(|step| step["fact"].as_str().unwrap().contains("`name: String`")),
        "{steps:?}"
    );
    assert!(
        steps
            .iter()
            .all(|step| !step["reason"].as_str().unwrap().is_empty()),
        "{steps:?}"
    );
    assert_eq!(held["gained"], true, "{held}");
    assert_eq!(held["panics"], false, "{held}");
    closed(client, handle);
}

/// A deduction that stops must reach the client as the same data shape:
/// the failing claim in the conclusion, the steps up to it, no gain.
#[test]
fn a_stopped_deduction_reaches_the_client_with_its_refusal() {
    let (client, handle) = opened("pub struct Batch {\n    pub held: Vec<String>,\n}\n");
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.deduce",
                "arguments": ["Batch", "Display"]
            })
        )))
        .unwrap();
    let Message::Response(answer) = client.receiver.recv().unwrap() else {
        panic!("the command must be answered");
    };
    let held = answer.response_result.as_ref().unwrap();
    assert!(
        held["conclusion"].as_str().unwrap().contains("`held`"),
        "{held}"
    );
    assert_eq!(held["gained"], false, "{held}");
    assert_eq!(held["panics"], serde_json::Value::Null, "{held}");
    closed(client, handle);
}
