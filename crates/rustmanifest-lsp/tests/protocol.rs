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
