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

/// What a compiler settled about one fixture, written down.
///
/// Every reading of the engine answers from a resolution and from nothing
/// else: what a value is, which type holds a field, which body a call
/// reaches. A session asking the server about a program therefore carries
/// the compiler's answers beside it, the way a checked-out workspace
/// carries the resolver's own emission. Each answer is named by the
/// written fragment it stands over and holds at every place the fixture
/// writes it.
struct Settled {
    /// File, relative to the workspace root, the fragments stand in.
    file: String,
    /// Text of that file.
    text: String,
    /// What has been written down so far.
    held: rustmanifest_schema::Resolution
}

impl Settled {
    /// Starts writing down what a compiler settled about `text`.
    fn over(file: &str, text: &str) -> Self {
        Self {
            file: file.to_owned(),
            text: text.to_owned(),
            held: rustmanifest_schema::Resolution {
                resolver: "test".to_owned(),
                ..rustmanifest_schema::Resolution::default()
            }
        }
    }

    /// Continues over a second file of the same workspace.
    fn and_over(mut self, file: &str, text: &str) -> Self {
        file.clone_into(&mut self.file);
        text.clone_into(&mut self.text);
        self
    }

    /// States the module tree, one row per mounted file.
    fn mounting(mut self, rows: &[(&str, &str)]) -> Self {
        for (names, file) in rows {
            self.held.modules.push(rustmanifest_schema::ResolvedModule {
                names:      (*names).to_owned(),
                file:       std::path::PathBuf::from(*file),
                crate_root: !names.contains("::"),
                parent:     names.rsplit_once("::").map(|(above, _)| above.to_owned())
            });
        }
        self
    }

    /// One location per place `fragment` is written.
    fn places(&self, fragment: &str) -> Vec<rustmanifest_schema::Location> {
        let mut found = Vec::new();
        let mut cursor = 0usize;
        while let Some(at) = self.text[cursor..].find(fragment) {
            let start = cursor + at;
            cursor = start + fragment.len();
            found.push(rustmanifest_schema::Location {
                file:  std::path::PathBuf::from(&self.file),
                range: rustmanifest_schema::Range {
                    start: u32::try_from(start).unwrap(),
                    end:   u32::try_from(cursor).unwrap()
                }
            });
        }
        assert!(!found.is_empty(), "the fixture writes {fragment:?} nowhere");
        found
    }

    /// States the type the compiler settled for everything written as
    /// `fragment`.
    fn typed(mut self, fragment: &str, rendered: &str) -> Self {
        for at in self.places(fragment) {
            self.held.types.push(rustmanifest_schema::ResolvedType {
                at,
                rendered: rendered.to_owned(),
                declared: None
            });
        }
        self
    }

    /// States the type holding every field written as `fragment` and the
    /// type that field carries.
    fn field(mut self, fragment: &str, owner: &str, produced: &str) -> Self {
        for at in self.places(fragment) {
            self.held.fields.push(rustmanifest_schema::ResolvedField {
                at,
                owner: owner.to_owned(),
                produced: produced.to_owned()
            });
        }
        self
    }

    /// States the body every call written as `fragment` reaches, what it
    /// hands back, and how it takes the value it stands on.
    fn call(
        mut self,
        fragment: &str,
        reaches: &str,
        returns: &str,
        takes: Option<rustmanifest_schema::ResolvedMode>
    ) -> Self {
        for at in self.places(fragment) {
            self.held.calls.push(rustmanifest_schema::ResolvedCall {
                at,
                callee: reaches.rsplit("::").next().unwrap_or(reaches).to_owned(),
                returns: returns.to_owned(),
                receiver: None,
                receiver_path: None,
                caller_decides: false,
                reaches: Some(reaches.to_owned()),
                takes
            });
        }
        self
    }

    /// States the full path of the body whose name is written as `fn
    /// <named>`.
    /// States that the compiler grants `named` the trait `carried`,
    /// whether a derive or a written impl put it there.
    fn granting(mut self, named: &str, carried: &str) -> Self {
        self.held.grants.push(rustmanifest_schema::ResolvedGrant {
            at:         rustmanifest_schema::Location {
                file:  std::path::PathBuf::from("src/lib.rs"),
                range: rustmanifest_schema::Range {
                    start: 0, end: 0
                }
            },
            named:      named.to_owned(),
            trait_name: carried.to_owned()
        });
        self
    }

    /// States the fields a compiler read off the declaration of `named`.
    ///
    /// What a field holds is the compiler's answer and nothing else, so
    /// a session asking about a type's shape states it here.
    fn shaping(mut self, named: &str, fields: &[(&str, &str)]) -> Self {
        self.held.declared.push(rustmanifest_schema::ResolvedTypeDecl {
            at:     rustmanifest_schema::Location {
                file:  std::path::PathBuf::from("src/lib.rs"),
                range: rustmanifest_schema::Range {
                    start: 0, end: 0
                }
            },
            named:  named.to_owned(),
            fields: fields
                .iter()
                .map(|(name, held)| rustmanifest_schema::ResolvedFieldDecl {
                    name:  (*name).to_owned(),
                    named: (*held).to_owned()
                })
                .collect()
        });
        self
    }

    fn declares(mut self, named: &str, path: &str) -> Self {
        let written = format!("fn {named}");
        let at = self.text.find(&written).expect("the fixture declares it");
        let start = at + written.len() - named.len();
        self.held
            .functions
            .push(rustmanifest_schema::ResolvedFunction {
                at:    rustmanifest_schema::Location {
                    file:  std::path::PathBuf::from(&self.file),
                    range: rustmanifest_schema::Range {
                        start: u32::try_from(start).unwrap(),
                        end:   u32::try_from(start + named.len()).unwrap()
                    }
                },
                names: path.to_owned()
            });
        self
    }

    /// Writes the resolution beside a fresh workspace root and hands the
    /// root back, the way a resolver's own run leaves it there.
    fn beside(mut self, files: &[(&str, &str)]) -> tempfile::TempDir {
        let bed = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let at = bed.path().join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        self.held.settle();
        std::fs::write(
            bed.path().join("resolution.json"),
            serde_json::to_string(&self.held).unwrap()
        )
        .unwrap();
        bed
    }
}

/// Opens one document of a rooted workspace over a fresh server and
/// swallows the published diagnostics, answering with the client and the
/// document's own uri.
fn opened_in(
    bed: &tempfile::TempDir,
    file: &str,
    text: &str
) -> (Connection, std::thread::JoinHandle<()>, String) {
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
    let uri = format!("file://{}", bed.path().join(file).display());
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
    (client, handle, uri)
}

/// A source the bundled rules flag: a hardcoded credential.
const FLAGGED: &str = "fn main() {\n    let password = \"hunter2\";\n}\n";

#[test]
fn an_opened_document_with_a_violation_receives_its_diagnostic() {
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
    handle.join().unwrap();
}

/// A source whose function clones, allocates, takes a borrowed
/// parameter, and reaches the file system only through a helper — the
/// memory story a card must tell, and the inference it must carry.
/// The workspace the hungry fixture stands in, with what a compiler
/// settled about it: which body the written call reaches, and the paths
/// the two declared bodies carry.
fn hungry_bed() -> tempfile::TempDir {
    Settled::over("src/lib.rs", HUNGRY)
        .mounting(&[("demo", "src/lib.rs")])
        .declares("persist", "demo::persist")
        .declares("hoard", "demo::hoard")
        .call("persist(input)", "demo::persist", "()", None)
        .beside(&[("src/lib.rs", HUNGRY)])
}

const HUNGRY: &str = "fn persist(entry: &str) { std::fs::write(\"log\", entry).ok(); }\nfn \
                      hoard(input: &str) -> Vec<String> {\n    let mut out = Vec::new();\n    \
                      out.push(input.to_owned());\n    out.push(format!(\"{input}\"));\n    \
                      persist(input);\n    out\n}\n";

#[test]
fn a_hover_on_a_function_name_answers_with_its_recorded_facts() {
    let bed = hungry_bed();
    let (client, handle, uri) = opened_in(&bed, "src/lib.rs", HUNGRY);
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/hover".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": uri },
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
    closed(client, handle);
}

/// The workspace command answers with the inference as data, so any
/// client scripts it.
#[test]
fn the_why_command_answers_with_the_inference_as_data() {
    let bed = hungry_bed();
    let (client, handle, uri) = opened_in(&bed, "src/lib.rs", HUNGRY);
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "workspace/executeCommand".to_owned(),
            serde_json::json!({
                "command": "rust-manifest.why",
                "arguments": [uri, "hoard", "fs"]
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
    closed(client, handle);
}

/// Two documents, the chain crossing between them: the card of the
/// caller in one file carries the inference down to the proof written
/// in the other.
#[test]
fn a_chain_crossing_two_open_documents_answers_whole() {
    let bed = chained_bed();
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
    let store_uri = format!("file://{}", bed.path().join("src/store.rs").display());
    let lib_uri = format!("file://{}", bed.path().join("src/lib.rs").display());
    for (uri, text) in [(store_uri.as_str(), CHAINED_STORE), (lib_uri.as_str(), CHAINED_LIB)] {
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
    closed(client, handle);
}

/// The interview scene itself: a function taking a `String` by value,
/// and a caller handing its local over. The hover on the written call
/// must tell the move.
/// A workspace whose call chain crosses two files, with the module tree a
/// resolver would state beside it.
fn chained_bed() -> tempfile::TempDir {
    Settled::over("src/store.rs", CHAINED_STORE)
        .mounting(&[("demo", "src/lib.rs"), ("demo::store", "src/store.rs")])
        .declares("persist", "demo::store::persist")
        .and_over("src/lib.rs", CHAINED_LIB)
        .declares("keep", "demo::keep")
        .call("crate::store::persist(entry)", "demo::store::persist", "()", None)
        .beside(&[
            ("src/store.rs", CHAINED_STORE),
            ("src/lib.rs", CHAINED_LIB)
        ])
}

/// The file the chain ends in: the write itself.
const CHAINED_STORE: &str = "pub fn persist(entry: &str) { std::fs::write(\"log\", entry).ok(); \
                             }\n";

/// The file the chain starts in: a call across the module boundary.
const CHAINED_LIB: &str = "mod store;\nfn keep(entry: &str) { crate::store::persist(entry); }\n";

const HANDED: &str = "fn foo(value: String) {}\nfn main() { let s = String::from(\"hello\"); \
                      foo(s); println!(\"{s}\"); }\n";

/// The workspace the interview scene stands in, with the identity a
/// compiler settles for it and nothing else: which body the written call
/// reaches is stated, and whether the type it hands over is copied is
/// not — which is exactly the question the surfaces must leave open.
fn handed_bed() -> tempfile::TempDir {
    Settled::over("src/lib.rs", HANDED)
        .mounting(&[("demo", "src/lib.rs")])
        .declares("foo", "demo::foo")
        .declares("main", "demo::main")
        .call("foo(s)", "demo::foo", "()", None)
        .beside(&[("src/lib.rs", HANDED)])
}

#[test]
fn a_hover_on_a_written_call_tells_where_ownership_goes() {
    let bed = handed_bed();
    let (client, handle, uri) = opened_in(&bed, "src/lib.rs", HANDED);
    let line = HANDED.lines().nth(1).unwrap();
    let character = line.find("foo(s)").unwrap() + 1;
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/hover".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": uri },
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
        "a resolution that does not say whether the type is copied must leave the question \
         open rather than claim an answer: {value}"
    );
    closed(client, handle);
}

/// A type and a trait it does not implement: the implement command must
/// answer with the written impl block, signatures from the declaration.
const UNWRITTEN: &str = "struct Disk;\ntrait Sink {\n    fn write(&mut self, chunk: &[u8]) -> \
                         usize;\n    fn flush(&mut self) {}\n}\n";

#[test]
fn the_implement_command_writes_the_impl_block() {
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
    closed(client, handle);
}

/// A cursor on the type's written name must offer the implementable
/// traits in the editor's action list, each carrying its edit.
#[test]
fn a_code_action_on_a_type_offers_the_unimplemented_traits() {
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
    closed(client, handle);
}

/// A type with fields, derives, and a written impl: the hover on its
/// name must card what it holds, implements, and weighs.
const TYPED: &str = "#[derive(Clone, Debug)]\nstruct User {\n    id: u64,\n    name: String,\n}\n\
                     impl std::fmt::Display for User {\n    fn fmt(&self, out: &mut \
                     std::fmt::Formatter<'_>) -> std::fmt::Result {\n        write!(out, \
                     \"{}\", self.name)\n    }\n}\n";

/// The workspace the type card stands in: what the compiler read off
/// the declaration, which is the only source of a field's type.
fn typed_bed() -> tempfile::TempDir {
    Settled::over("src/lib.rs", TYPED)
        .shaping("User", &[("id", "u64"), ("name", "String")])
        .granting("User", "Clone")
        .granting("User", "Debug")
        .granting("User", "Display")
        .beside(&[("src/lib.rs", TYPED)])
}

#[test]
fn a_hover_on_a_type_name_cards_its_facts() {
    let bed = typed_bed();
    let (client, handle, uri) = opened_in(&bed, "src/lib.rs", TYPED);
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
                "textDocument": { "uri": uri },
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
        value.contains("✓ `Clone` — the compiler says the type carries it"),
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
    let bed = handed_bed();
    let (client, handle, uri) = opened_in(&bed, "src/lib.rs", HANDED);
    client
        .sender
        .send(Message::Request(Request::new(
            2.into(),
            "textDocument/inlayHint".to_owned(),
            serde_json::json!({
                "textDocument": { "uri": uri },
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
        "a resolution that does not say whether the type is copied leaves the question open: \
         {hints:?}"
    );
    let line = HANDED.lines().nth(1).unwrap();
    let argument = line.find("foo(s)").unwrap() + 4;
    assert_eq!(hints[0]["position"]["line"], 1, "{hints:?}");
    assert_eq!(hints[0]["position"]["character"], argument, "{hints:?}");
    closed(client, handle);
}

/// A finding that carries a synthesized fix must offer it as a
/// quickfix action, edits included.
const FIXABLE: &str = "struct Report { total: u64 }\nfn summarize(report: Report) -> u64 { \
                       report.total }\n";

#[test]
fn a_diagnostic_with_a_fix_offers_it_as_a_quickfix() {
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
    closed(client, handle);
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
    let text = "use std::collections::BTreeMap;\n\npub struct Task {\n    pub title: \
                String,\n}\n\npub struct Store {\n    pub tasks: BTreeMap<u64, \
                Task>,\n}\n\nimpl Store {\n    pub fn add(&mut self, title: String) {\n        \
                let id = self.tasks.len() as u64;\n        self.tasks.insert(id, Task { title \
                });\n    }\n}\n";
    let bed = Settled::over("src/lib.rs", text)
        .mounting(&[("demo", "src/lib.rs")])
        .field("self.tasks", "Store", "BTreeMap<u64, Task>")
        .typed("self.tasks", "BTreeMap<u64, Task>")
        .beside(&[("src/lib.rs", text)]);
    let (client, handle, _uri) = opened_in(&bed, "src/lib.rs", text);
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
    let held = "mod ledger;\nmod api;\npub struct Wallet { pub balance: i64 }\n";
    let posted = "use crate::Wallet;\npub fn post(wallet: &mut Wallet, amount: i64) { \
                  wallet.balance += amount; }\n";
    let hacked = "use crate::Wallet;\npub fn hack(wallet: &mut Wallet) { wallet.balance = 0; }\n";
    let invariant = "[[invariants]]\nid = \"WALLET-BALANCE\"\nfield = \
                     \"Wallet.balance\"\nwrite-only-from = [\"crate::ledger\"]\n";
    let bed = Settled::over("src/ledger.rs", posted)
        .mounting(&[
            ("demo", "src/lib.rs"),
            ("demo::ledger", "src/ledger.rs"),
            ("demo::api", "src/api.rs")
        ])
        .field("wallet.balance", "Wallet", "i64")
        .and_over("src/api.rs", hacked)
        .field("wallet.balance", "Wallet", "i64")
        .beside(&[
            ("src/lib.rs", held),
            ("src/ledger.rs", posted),
            ("src/api.rs", hacked),
            ("rustmanifest.toml", invariant)
        ]);
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
