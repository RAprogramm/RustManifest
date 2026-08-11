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
//! facts cannot carry one. `rust-manifest.deduce` takes the same pair
//! and answers with the deduction chain itself — conclusion and steps,
//! the workspace's own precedent advising after the catalog's choice —
//! with the block installed in memory and confirmed by re-reading, so
//! the answer says not only what to write but why every claim holds;
//! no byte touches a disk. `rust-manifest.intents` takes nothing and
//! answers with every intent the workspace's own written calls prove,
//! each citing the call as evidence and carrying the call's written
//! address — the citing document's URI and the calling construct's
//! positions — so the citation is a place to jump to, not a string to
//! search for. `rust-manifest.whence` takes a function name and
//! answers with the question layer's second form: where the value the
//! function hands back comes from, each written position cited with
//! its byte range and restated across calls; given [uri, line,
//! character] instead, it answers for the innermost recorded position
//! covering that written place — the same question asked of any byte. The
//! answer is the command's result, so any
//! client scripts it: an editor plugin, a keybinding, a pipeline.
//! Nothing is shown or inserted by the server itself — how to use an
//! answer is the asker's decision.

use lsp_server::{Request, Response};

use super::{position::LineIndex, snapshot::Derived};

/// The inference command this server serves.
pub(super) const WHY: &str = "rust-manifest.why";

/// The generating command this server serves.
pub(super) const IMPLEMENT: &str = "rust-manifest.implement";

/// The deducing command this server serves.
pub(super) const DEDUCE: &str = "rust-manifest.deduce";

/// The intent-listing command this server serves.
pub(super) const INTENTS: &str = "rust-manifest.intents";

/// The value-origin command this server serves.
pub(super) const WHENCE: &str = "rust-manifest.whence";

/// The store-purpose command this server serves.
pub(super) const KEPT: &str = "rust-manifest.kept";

/// Answers one `workspace/executeCommand` request over the snapshot.
pub(super) fn answer(request: &Request, held: &Derived<'_>) -> Response {
    if request.params["command"].as_str() == Some(IMPLEMENT) {
        return implemented(request, held);
    }
    if request.params["command"].as_str() == Some(DEDUCE) {
        return deduced(request, held);
    }
    if request.params["command"].as_str() == Some(INTENTS) {
        return intended(request, held);
    }
    if request.params["command"].as_str() == Some(WHENCE) {
        return sourced(request, held);
    }
    if request.params["command"].as_str() == Some(KEPT) {
        return held_for(request, held);
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
            "refused": asked.refused,
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

/// Answers the whence command: where the value the asked function
/// hands back comes from — the question layer's second form, the same
/// chain-of-steps shape the why command answers with.
fn sourced(request: &Request, held: &Derived<'_>) -> Response {
    let arguments = &request.params["arguments"];
    let Some(first) = arguments[0].as_str() else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("`{WHENCE}` takes [function] or [uri, line, character]")
        );
    };
    let positioned = (arguments[1].as_u64(), arguments[2].as_u64());
    let asked = if let (Some(line), Some(character)) = positioned {
        let path = super::snapshot::path_of(first);
        let Some(file) = held.sources.iter().position(|source| source.path() == path) else {
            return Response::new_err(
                request.id.clone(),
                lsp_server::ErrorCode::InvalidParams as i32,
                format!("`{WHENCE}` knows no source at `{first}`")
            );
        };
        let text = held.sources[file].text();
        let offset = super::position::offset_of(
            text,
            lsp_types::Position {
                line:      u32::try_from(line).unwrap_or(u32::MAX),
                character: u32::try_from(character).unwrap_or(u32::MAX)
            }
        );
        rustmanifest_engine::semantic::whence_at_over(
            held.sources,
            held.facts,
            held.graph,
            file,
            u32::try_from(offset).unwrap_or(u32::MAX)
        )
    } else {
        rustmanifest_engine::semantic::whence_value_over(
            held.sources,
            held.facts,
            held.graph,
            first
        )
    };
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
            "refused": asked.refused,
            "steps": steps
        })
    )
}

/// Answers the kept command: why the program keeps the asked store —
/// the question layer's third form, the same chain-of-steps shape the
/// why command answers with.
fn held_for(request: &Request, held: &Derived<'_>) -> Response {
    let arguments = &request.params["arguments"];
    let Some(store) = arguments[0].as_str() else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("`{KEPT}` takes [store]")
        );
    };
    let workspace = rustmanifest_engine::Workspace::new(held.sources);
    let asked = rustmanifest_engine::semantic::why_kept(&workspace, store);
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
            "refused": asked.refused,
            "steps": steps
        })
    )
}

/// Answers the intents command: every intent the workspace's own
/// written calls prove, each with the call standing as evidence — the
/// program names what is missing before anyone asks.
fn intended(request: &Request, held: &Derived<'_>) -> Response {
    let derived: Vec<serde_json::Value> =
        rustmanifest_engine::semantic::derived_intents_with(held.facts, held.types)
            .into_iter()
            .map(|intent| {
                serde_json::json!({
                    "type": intent.type_name,
                    "trait": intent.trait_name,
                    "asked_by": intent.asked_by,
                    "call": intent.call,
                    "site": sited(held, &intent),
                    "evidence": {
                        "fact": intent.evidence.fact,
                        "reason": intent.evidence.reason
                    }
                })
            })
            .collect();
    Response::new_ok(
        request.id.clone(),
        serde_json::json!({ "intents": derived })
    )
}

/// The written address of an intent's evidence, spelled the way the
/// protocol spells locations: the citing document's URI and the
/// calling construct's positions in it — so a client jumps to the
/// site instead of searching for the quoted text.
fn sited(
    held: &Derived<'_>,
    intent: &rustmanifest_engine::semantic::DerivedIntent
) -> serde_json::Value {
    let Some(source) = held.sources.get(intent.source) else {
        return serde_json::Value::Null;
    };
    let text = source.text();
    let index = LineIndex::new(text);
    serde_json::json!({
        "uri": format!("file://{}", source.path().display()),
        "range": {
            "start": index.position(text, intent.site.start as usize),
            "end": index.position(text, intent.site.end as usize)
        }
    })
}

/// Answers the deduce command: the chain as data — conclusion, steps,
/// whether the re-reading confirmed the gain, and whether an installed
/// body still carries a panic obligation.
fn deduced(request: &Request, held: &Derived<'_>) -> Response {
    let arguments = &request.params["arguments"];
    let (Some(type_name), Some(trait_name)) = (arguments[0].as_str(), arguments[1].as_str())
    else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("`{DEDUCE}` takes [type, trait]")
        );
    };
    let chained = rustmanifest_learn::deduce_with_precedent(held.sources, type_name, trait_name)
        .unwrap_or_else(|| {
            rustmanifest_engine::semantic::deduce_impl(held.sources, 0, type_name, trait_name)
        });
    let steps: Vec<serde_json::Value> = chained
        .chain
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
            "conclusion": chained.chain.conclusion,
            "steps": steps,
            "gained": chained.gain.is_some(),
            "panics": chained.gain.map(|gain| gain.panics)
        })
    )
}
