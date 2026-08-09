// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The workspace command: the question layer over the wire, as data.
//!
//! `rust-manifest.why` takes a document, a function name and an effect,
//! and answers with the same inference the CLI prints — conclusion and
//! steps, each step a recorded fact with its reason. The answer is the
//! command's result, so any client scripts it: an editor plugin, a
//! keybinding, a pipeline. Nothing is shown by the server itself —
//! how to render an answer is the asker's decision.

use lsp_server::{Request, Response};

/// The one command this server serves.
pub(super) const WHY: &str = "rust-manifest.why";

/// Answers one `workspace/executeCommand` request from the stored text.
pub(super) fn answer(
    request: &Request,
    documents: &std::collections::BTreeMap<String, String>
) -> Response {
    let arguments = &request.params["arguments"];
    let (Some(uri), Some(function), Some(effect)) = (
        arguments[0].as_str(),
        arguments[1].as_str(),
        arguments[2].as_str()
    ) else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("`{WHY}` takes [uri, function, effect]")
        );
    };
    let Some(held) = documents.get(uri) else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("no opened document is named `{uri}`")
        );
    };
    let path = std::path::PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri));
    let sources = [rustmanifest_engine::Source::new(path, held.clone())];
    let workspace = rustmanifest_engine::Workspace::new(&sources);
    let asked = rustmanifest_engine::semantic::why_effect(&workspace, function, effect);
    let steps: Vec<serde_json::Value> = asked
        .steps
        .iter()
        .map(|step| {
            serde_json::json!({
                "fact": step.fact,
                "reason": step.reason
            })
        })
        .collect();
    Response::new_ok(
        request.id.clone(),
        serde_json::json!({
            "conclusion": asked.conclusion,
            "steps": steps
        })
    )
}
