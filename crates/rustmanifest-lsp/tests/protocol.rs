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
    let bed = chained_bed();
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server));
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({
                "capabilities": {},
                "workspaceFolders": [{
                    "uri": format!("file://{}", bed.path().display()),
                    "name": "bed"
                }]
            })
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
    let store_uri = format!("file://{}", bed.path().join("src/store.rs").display());
    let lib_uri = format!("file://{}", bed.path().join("src/lib.rs").display());
    for (uri, text) in [
        (
            store_uri.as_str(),
            "pub fn persist(entry: &str) { std::fs::write(\"log\", entry).ok(); }\n"
        ),
        (
            lib_uri.as_str(),
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
                "arguments": [lib_uri.as_str(), "keep", "fs"]
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
/// A workspace whose call chain crosses two files, with the module tree a
/// resolver would state beside it.
fn chained_bed() -> tempfile::TempDir {
    let bed = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(bed.path().join("src")).unwrap();
    std::fs::write(
        bed.path().join("src/store.rs"),
        "pub fn persist(entry: &str) { std::fs::write(\"log\", entry).ok(); }\n"
    )
    .unwrap();
    std::fs::write(
        bed.path().join("src/lib.rs"),
        "mod store;\nfn keep(entry: &str) { crate::store::persist(entry); }\n"
    )
    .unwrap();
    settled_tree(
        bed.path(),
        &[("demo", "src/lib.rs"), ("demo::store", "src/store.rs")]
    );
    bed
}

/// Writes beside the workspace the module tree a resolver would state,
/// which the server reads on the next build.
fn settled_tree(root: &std::path::Path, rows: &[(&str, &str)]) {
    let modules: Vec<String> = rows
        .iter()
        .map(|(names, file)| {
            let root_module = !names.contains("::");
            let parent = names.rsplit_once("::").map_or(String::new(), |(above, _)| {
                format!(", \"parent\": \"{above}\"")
            });
            format!(
                "{{\"names\": \"{names}\", \"file\": \"{file}\", \"crate-root\": \
                 {root_module}{parent}}}"
            )
        })
        .collect();
    std::fs::write(
        root.join("resolution.json"),
        format!(
            "{{\"resolver\": \"test\", \"calls\": [], \"modules\": [{}]}}",
            modules.join(", ")
        )
    )
    .unwrap();
}

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
    assert!(published.params["diagnostics"].is_array());
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
        value.contains("`value: String` is taken by value"),
        "{value}"
    );
    assert!(
        value.contains("unsettled"),
        "a session with no resolution beside it must say the question is open rather than \
         claim an answer: {value}"
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
        value.contains("`Copy` — not derived"),
        "without a resolution beside the session, which field forbids the copy is the \
         compiler's answer and stays unclaimed: {value}"
    );
    assert!(
        value.contains("– `Default` — not written in the workspace"),
        "{value}"
    );
    assert!(value.contains("are the compiler's answer"), "{value}");
    assert!(
        !value.contains("byte(s) on the stack"),
        "without a resolution beside the session what a value weighs is the compiler's answer \
         and stays unclaimed: {value}"
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
    assert_eq!(
        hints[0]["label"], "by value?",
        "a session with no resolution beside it states the open question: {hints:?}"
    );
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

/// The intents command must answer with what the workspace's own
/// written calls prove is missing, each intent citing its evidence
/// and the evidence carrying its written address — a place to jump
/// to, not a string to search for.
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
    let site = &intents[0]["site"];
    assert_eq!(site["uri"], "file:///demo/src/lib.rs", "{intents:?}");
    assert_eq!(site["range"]["start"]["line"], 5, "{intents:?}");
    assert_eq!(site["range"]["start"]["character"], 4, "{intents:?}");
    assert_eq!(site["range"]["end"]["line"], 5, "{intents:?}");
    closed(client, handle);
}

/// The whence command answers the question layer's second form: the
/// value's origin restated across calls, steps carrying byte-ranged
/// citations of the written positions.
#[test]
fn the_whence_command_restates_the_value_across_calls() {
    let (client, handle) =
        opened("fn seed() -> u32 {\n    7\n}\n\npub fn make() -> u32 {\n    seed()\n}\n");
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.whence",
                "arguments": ["make"]
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
            .contains("the written value"),
        "{held:?}"
    );
    let steps = held["steps"].as_array().unwrap();
    assert!(
        steps.iter().any(|step| step["fact"]
            .as_str()
            .unwrap()
            .contains("restated across calls")),
        "{held:?}"
    );
    closed(client, handle);
}

/// The kept command answers the question layer's third form: the
/// store's declaration and every proven verb, the chain shape shared
/// by every question command.
#[test]
fn the_kept_command_answers_why_a_store_is_kept() {
    let (client, handle) = opened(
        "use std::collections::BTreeMap;\n\npub struct Task {\n    pub title: String,\n}\n\npub \
         struct Store {\n    pub tasks: BTreeMap<u64, Task>,\n}\n\nimpl Store {\n    pub fn \
         add(&mut self, title: String) {\n        let id = self.tasks.len() as u64;\n        \
         self.tasks.insert(id, Task { title });\n    }\n}\n"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.kept",
                "arguments": ["Store.tasks"]
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
            .contains("`Task` records"),
        "{held:?}"
    );
    let steps = held["steps"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step["fact"].as_str().unwrap().contains("adds a record")),
        "{held:?}"
    );
    closed(client, handle);
}

/// The whence command's positional form answers for a byte: the
/// cursor on a written call names what the callee hands back.
#[test]
fn the_whence_command_answers_for_a_position() {
    let (client, handle) = opened(
        "fn seed() -> u32 {\n    7\n}\n\npub fn make() -> u32 {\n    let held = seed();\n             held\n}\n"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.whence",
                "arguments": ["file:///demo/src/lib.rs", 5, 16]
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
            .contains("the written value"),
        "{held:?}"
    );
    assert!(
        held["steps"][0]["fact"]
            .as_str()
            .unwrap()
            .contains("`seed` hands back"),
        "{held:?}"
    );
    closed(client, handle);
}

/// An opened document whose written call states an intent receives the
/// intent as a diagnostic on the call site, its related information
/// leading to the cited type's declaration — a jump from either end,
/// not a title to read.
#[test]
fn an_opened_document_with_a_written_intent_receives_its_diagnostic() {
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
                    "text": "pub struct Job {\n    pub name: String,\n}\n\npub fn make() -> Job \
                             {\n    Job::default()\n}\n"
                }
            })
        )))
        .unwrap();
    let Message::Notification(published) = client.receiver.recv().unwrap() else {
        panic!("the first message after an open must be the diagnostics");
    };
    let diagnostics = published.params["diagnostics"].as_array().unwrap().clone();
    let intent = diagnostics
        .iter()
        .find(|entry| entry["code"] == "intent")
        .unwrap_or_else(|| panic!("{diagnostics:?}"));
    assert_eq!(intent["source"], "rust-manifest", "{intent:?}");
    assert!(
        intent["message"].as_str().unwrap().contains("Job::default"),
        "{intent:?}"
    );
    assert_eq!(intent["range"]["start"]["line"], 5, "{intent:?}");
    assert_eq!(intent["range"]["start"]["character"], 4, "{intent:?}");
    let related = &intent["relatedInformation"][0];
    assert_eq!(
        related["location"]["uri"], "file:///demo/src/lib.rs",
        "{intent:?}"
    );
    assert_eq!(
        related["location"]["range"]["start"]["line"], 0,
        "{intent:?}"
    );
    assert_eq!(
        related["location"]["range"]["start"]["character"], 11,
        "{intent:?}"
    );
    assert!(
        related["message"].as_str().unwrap().contains("`Job`"),
        "{intent:?}"
    );
    closed(client, handle);
}

/// A declared workspace invariant lands as a diagnostic where its
/// violation stands, the message carrying the derivation trace — the
/// first workspace invariant delivered to the editor as a diagnostic.
#[test]
fn a_declared_invariant_violation_receives_its_diagnostic_with_the_trace() {
    let bed = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(bed.path().join("src")).unwrap();
    std::fs::write(
        bed.path().join("src/lib.rs"),
        "mod ledger;\nmod api;\npub struct Wallet { pub balance: i64 }\n"
    )
    .unwrap();
    std::fs::write(
        bed.path().join("src/ledger.rs"),
        "use crate::Wallet;\npub fn post(wallet: &mut Wallet, amount: i64) { wallet.balance += \
         amount; }\n"
    )
    .unwrap();
    let hacked = "use crate::Wallet;\npub fn hack(wallet: &mut Wallet) { wallet.balance = 0; }\n";
    std::fs::write(bed.path().join("src/api.rs"), hacked).unwrap();
    std::fs::write(
        bed.path().join("rustmanifest.toml"),
        "[[invariants]]\nid = \"WALLET-BALANCE\"\nfield = \"Wallet.balance\"\nwrite-only-from = \
         [\"crate::ledger\"]\n"
    )
    .unwrap();
    settled_tree(
        bed.path(),
        &[
            ("demo", "src/lib.rs"),
            ("demo::ledger", "src/ledger.rs"),
            ("demo::api", "src/api.rs")
        ]
    );
    let (server, client) = Connection::memory();
    let handle = std::thread::spawn(move || rustmanifest_lsp::serve(&server).unwrap());
    client
        .sender
        .send(Message::Request(Request::new(
            1.into(),
            "initialize".to_owned(),
            serde_json::json!({
                "capabilities": {},
                "workspaceFolders": [{
                    "uri": format!("file://{}", bed.path().display()),
                    "name": "bed"
                }]
            })
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
                    "uri": format!("file://{}", bed.path().join("src/api.rs").display()),
                    "languageId": "rust",
                    "version": 1,
                    "text": hacked
                }
            })
        )))
        .unwrap();
    let Message::Notification(published) = client.receiver.recv().unwrap() else {
        panic!("the first message after an open must be the diagnostics");
    };
    let diagnostics = published.params["diagnostics"].as_array().unwrap().clone();
    let violated = diagnostics
        .iter()
        .find(|entry| entry["code"] == "WALLET-BALANCE")
        .unwrap_or_else(|| panic!("{diagnostics:?}"));
    assert_eq!(violated["source"], "rust-manifest", "{violated:?}");
    assert_eq!(violated["severity"], 1, "{violated:?}");
    let message = violated["message"].as_str().unwrap();
    assert!(
        message.contains("`hack` writes `Wallet.balance` from module `crate::api`"),
        "{message}"
    );
    assert!(message.contains("\n• "), "{message}");
    assert_eq!(violated["range"]["start"]["line"], 1, "{violated:?}");
    closed(client, handle);
}

/// An action backed by a derived intent cites the asking call in its
/// title: the author sees not only what can be installed but who in
/// their own code already asked for it.
#[test]
fn an_intent_backed_action_cites_the_asking_call() {
    let (client, handle) = opened(
        "pub struct Job {\n    pub name: String,\n}\n\npub fn make() -> Job {\n    \
         Job::default()\n}\n"
    );
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/codeAction".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": "file:///demo/src/lib.rs" },
                "range": {
                    "start": { "line": 0, "character": 12 },
                    "end": { "line": 0, "character": 12 }
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
        .filter_map(|action| action["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"implement Default for Job — fn make calls Job::default"),
        "{titles:?}"
    );
    assert!(
        titles.contains(&"implement Display for Job"),
        "an intent must not decorate a trait nobody asked for: {titles:?}"
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
