// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Protocol conformance for the stdio MCP server.
//!
//! The compiled server binary is spawned with a temporary workspace as its
//! current directory; a scripted JSON-RPC session is written to stdin, the
//! transcript is read back from stdout after EOF, and every response is
//! checked structurally: handshake, tool listing, resource listing and
//! reading, a real review with findings and traces, path-escape rejection,
//! and unknown-tool errors.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Stdio},
    time::Duration
};

use serde_json::{Value, json};

/// Kills the server if the scripted session hangs.
const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs one scripted stdio session and returns responses keyed by id.
fn session(requests: &[Value]) -> BTreeMap<i64, Value> {
    let workspace = tempfile::tempdir().expect("workspace dir");
    std::fs::write(
        workspace.path().join("probe.rs"),
        "pub struct ConfigLoader {\n    path: String\n}\n"
    )
    .expect("probe file");
    std::fs::write(
        workspace.path().join("logic.rs"),
        "pub fn shout(text: &str) {\n    println!(\"{text}\");\n}\n"
    )
    .expect("logic file");

    let mut child = Command::new(env!("CARGO_BIN_EXE_rustmanifest-mcp"))
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server spawns");

    let mut stdin = child.stdin.take().expect("stdin handle");
    let mut script = String::new();
    for request in requests {
        script.push_str(&request.to_string());
        script.push('\n');
    }
    stdin.write_all(script.as_bytes()).expect("script written");
    drop(stdin);

    let watchdog = {
        let pid = child.id();
        std::thread::spawn(move || {
            std::thread::sleep(SESSION_TIMEOUT);
            Command::new("kill").arg(pid.to_string()).status().ok();
        })
    };
    let output = child.wait_with_output().expect("server exits");
    drop(watchdog);

    let mut responses = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(id) = message.get("id").and_then(Value::as_i64) {
            responses.insert(id, message);
        }
    }
    responses
}

/// The standard scripted session shared by every assertion below.
fn scripted() -> BTreeMap<i64, Value> {
    session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "conformance", "version": "0"}
        }}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "resources/read", "params": {
            "uri": "rustmanifest://methodology/structure"
        }}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {
            "name": "review_file", "arguments": {"path": "probe.rs"}
        }}),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {
            "name": "review_file", "arguments": {"path": "../outside.rs"}
        }}),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {
            "name": "no_such_tool", "arguments": {}
        }}),
        json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {
            "name": "project_overview", "arguments": {"path": "."}
        }}),
        json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {
            "name": "function_facts", "arguments": {"path": "logic.rs"}
        }})
    ])
}

#[test]
fn handshake_reports_server_identity_and_capabilities() {
    let responses = scripted();
    let init = &responses[&1]["result"];
    assert!(init["protocolVersion"].is_string());
    assert!(init["capabilities"]["tools"].is_object());
    assert!(init["capabilities"]["resources"].is_object());
    assert!(init["serverInfo"]["name"].is_string());
}

#[test]
fn tools_list_exposes_exactly_the_nine_tools() {
    let responses = scripted();
    let mut names: Vec<&str> = responses[&2]["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "check_rule",
            "explain_rule",
            "function_facts",
            "learn_conventions",
            "lint_structure",
            "project_overview",
            "review_file",
            "review_path",
            "suppressions"
        ]
    );
}

#[test]
fn project_overview_renders_the_knowledge_report() {
    let responses = scripted();
    let text = responses[&8]["result"]["content"][0]["text"]
        .as_str()
        .expect("overview text");
    assert!(text.contains("# Project overview"));
    assert!(text.contains("## Behavioral roles"));
}

#[test]
fn function_facts_report_roles() {
    let responses = scripted();
    let text = responses[&9]["result"]["content"][0]["text"]
        .as_str()
        .expect("facts text");
    assert!(text.contains("\"role\""));
}

#[test]
fn resources_list_exposes_the_methodology_documents() {
    let responses = scripted();
    let resources = responses[&3]["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert_eq!(resources.len(), 9);
    for resource in resources {
        let uri = resource["uri"].as_str().expect("resource uri");
        assert!(uri.starts_with("rustmanifest://methodology/"));
    }
}

#[test]
fn resources_read_returns_the_structure_document() {
    let responses = scripted();
    let contents = responses[&4]["result"]["contents"]
        .as_array()
        .expect("contents array");
    let text = contents[0]["text"].as_str().expect("document text");
    assert!(text.contains("Entity Naming"));
}

#[test]
fn review_file_reports_findings_with_traces() {
    let responses = scripted();
    let findings = responses[&5]["result"]["structuredContent"]["findings"]
        .as_array()
        .expect("findings array");
    assert!(!findings.is_empty());
    let entity = findings
        .iter()
        .find(|finding| finding["rule-id"] == "RM-STRUCT-001")
        .expect("entity naming finding");
    assert!(
        entity["rationale-uri"]
            .as_str()
            .expect("rationale uri")
            .starts_with("rustmanifest://methodology/")
    );
    assert!(!entity["trace"].as_array().expect("trace array").is_empty());
}

#[test]
fn path_escape_is_rejected() {
    let responses = scripted();
    let message = responses[&6]["error"]["message"]
        .as_str()
        .expect("error message");
    assert!(message.contains("escapes"));
}

#[test]
fn unknown_tool_is_an_error() {
    let responses = scripted();
    assert!(responses[&7].get("error").is_some());
}

/// A workspace carrying the evaluation bed serves it: the corpus
/// manifest and the blessed baseline appear as resources and read back
/// verbatim; the plain workspace above lists none of them.
#[test]
fn a_workspace_with_the_evaluation_bed_serves_its_measurement() {
    let workspace = tempfile::tempdir().expect("workspace dir");
    std::fs::create_dir(workspace.path().join("eval")).expect("eval dir");
    std::fs::write(
        workspace.path().join("eval/corpus.toml"),
        "[[repo]]\nname = \"probe\"\npath = \"probe\"\nrev = \"0\"\n"
    )
    .expect("corpus manifest");
    std::fs::write(
        workspace.path().join("eval/baseline.json"),
        "{\"repos\":{}}"
    )
    .expect("baseline");
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustmanifest-mcp"))
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server spawns");
    let mut stdin = child.stdin.take().expect("stdin handle");
    let mut script = String::new();
    for request in [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "conformance", "version": "0"}
        }}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": {
            "uri": "rustmanifest://corpus/baseline"
        }})
    ] {
        script.push_str(&request.to_string());
        script.push('\n');
    }
    stdin.write_all(script.as_bytes()).expect("script written");
    drop(stdin);
    let watchdog = {
        let pid = child.id();
        std::thread::spawn(move || {
            std::thread::sleep(SESSION_TIMEOUT);
            Command::new("kill").arg(pid.to_string()).status().ok();
        })
    };
    let output = child.wait_with_output().expect("server exits");
    drop(watchdog);
    let mut responses = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(id) = message.get("id").and_then(Value::as_i64) {
            responses.insert(id, message);
        }
    }
    let listed = responses[&2]["result"]["resources"]
        .as_array()
        .expect("resources array");
    let corpus: Vec<&str> = listed
        .iter()
        .filter_map(|resource| resource["uri"].as_str())
        .filter(|uri| uri.starts_with("rustmanifest://corpus/"))
        .collect();
    assert_eq!(
        corpus,
        [
            "rustmanifest://corpus/manifest",
            "rustmanifest://corpus/baseline"
        ],
        "the bed's two present files must be served, the absent report must not"
    );
    let read = responses[&3]["result"]["contents"][0]["text"]
        .as_str()
        .expect("baseline text");
    assert_eq!(read, "{\"repos\":{}}");
}

/// A workspace without the bed keeps the corpus URIs out of the
/// listing entirely: absent measurement, absent claims.
#[test]
fn a_workspace_without_the_bed_lists_no_corpus_resources() {
    let responses = scripted();
    let listed = responses[&3]["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert!(
        listed
            .iter()
            .filter_map(|resource| resource["uri"].as_str())
            .all(|uri| !uri.starts_with("rustmanifest://corpus/"))
    );
}
