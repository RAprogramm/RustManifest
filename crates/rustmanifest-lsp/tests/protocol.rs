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
    assert!(value.contains("### `hoard`"), "{value}");
    assert!(value.contains("clones: 1"), "{value}");
    assert!(value.contains("allocations:"), "{value}");
    assert!(value.contains("taken borrowed"), "{value}");
    assert!(
        value.contains("effects reached through calls: `fs`"),
        "{value}"
    );
    assert!(value.contains("why `fs`:"), "{value}");
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
                      foo(s); }\n";

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
    let _diagnostics = client.receiver.recv().unwrap();
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
    assert!(value.contains("### ownership at this call"), "{value}");
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
