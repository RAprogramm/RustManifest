// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The workspace commands: the question layer over the wire, as data.
//!
//! `rust-manifest.why` takes a document, a function name and an effect,
//! and answers with the same inference the CLI prints — conclusion and
//! steps, each step a recorded fact with its reason — computed over the
//! whole workspace snapshot, so the chain follows calls across files.
//! `rust-manifest.implement` takes a type name and a trait name and
//! answers with the written `impl` block, or with nothing where the
//! facts cannot carry one. The answer is the command's result, so any
//! client scripts it: an editor plugin, a keybinding, a pipeline.
//! Nothing is shown or inserted by the server itself — how to use an
//! answer is the asker's decision.

use lsp_server::{Request, Response};

use super::snapshot::Derived;

/// The inference command this server serves.
pub(super) const WHY: &str = "rust-manifest.why";

/// The generating command this server serves.
pub(super) const IMPLEMENT: &str = "rust-manifest.implement";

/// Answers one `workspace/executeCommand` request over the snapshot.
pub(super) fn answer(request: &Request, held: &Derived<'_>) -> Response {
    if request.params["command"].as_str() == Some(IMPLEMENT) {
        return implemented(request, held);
    }
    let arguments = &request.params["arguments"];
    let (Some(_uri), Some(function), Some(effect)) = (
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
    let asked = rustmanifest_engine::semantic::why_effect_over(
        held.sources,
        held.facts,
        held.graph,
        function,
        effect
    );
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

/// Answers the implement command: the written `impl` block, or a null
/// text where the facts cannot carry one.
fn implemented(request: &Request, held: &Derived<'_>) -> Response {
    let arguments = &request.params["arguments"];
    let (Some(type_name), Some(trait_name)) = (arguments[0].as_str(), arguments[1].as_str())
    else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("`{IMPLEMENT}` takes [type, trait]")
        );
    };
    let written = rustmanifest_engine::semantic::impl_skeleton(
        held.sources,
        held.types,
        type_name,
        trait_name
    );
    Response::new_ok(request.id.clone(), serde_json::json!({ "text": written }))
}
