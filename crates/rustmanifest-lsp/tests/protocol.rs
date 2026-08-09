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

/// A source whose function clones, allocates and takes a borrowed
/// parameter — the memory story a card must tell.
const HUNGRY: &str = "fn hoard(input: &str) -> Vec<String> {\n    let mut out = Vec::new();\n    \
                      out.push(input.to_owned());\n    out.push(format!(\"{input}\"));\n    \
                      std::fs::write(\"log\", input).ok();\n    out\n}\n";

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
                "position": { "line": 0, "character": 4 }
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
    assert!(value.contains("effects of its own body: `fs`"), "{value}");
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
