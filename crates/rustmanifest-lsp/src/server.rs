// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The protocol loop: initialize, then diagnostics on every open,
//! change and save.
//!
//! Full-text synchronization is deliberate for the first shipped
//! surface: the engine analyzes a whole file in around a millisecond,
//! so incremental edits would buy latency nobody is waiting on at the
//! price of a reconciliation the protocol is famous for getting wrong.
//! Open and save additionally publish the document's intent
//! diagnostics and the violations of the root's declared invariants,
//! both derived over the whole workspace snapshot; a keystroke
//! republishes only the per-document findings, so typing never pays
//! for a workspace walk.

use anyhow::Context;
use lsp_server::{Connection, Message, Notification};
use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    PublishDiagnosticsParams, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
    notification::{
        DidChangeTextDocument, DidOpenTextDocument, DidSaveTextDocument, Notification as _,
        PublishDiagnostics
    }
};

mod action;
mod analysis;
mod card;
mod command;
mod hint;
mod hover;
mod intent;
mod invariant;
mod position;
mod snapshot;

use std::collections::BTreeMap;

use self::{analysis::Analysis, snapshot::Snapshot};

/// Runs the server over `connection` until the client asks to shut down.
///
/// # Errors
///
/// Returns protocol failures: a broken transport, or a handshake the
/// client abandoned.
pub fn serve(connection: &Connection) -> anyhow::Result<()> {
    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
        inlay_hint_provider: Some(lsp_types::OneOf::Left(true)),
        execute_command_provider: Some(lsp_types::ExecuteCommandOptions {
            commands: vec![
                command::WHY.to_owned(),
                command::IMPLEMENT.to_owned(),
                command::DEDUCE.to_owned(),
                command::INTENTS.to_owned(),
                command::WHENCE.to_owned(),
                command::KEPT.to_owned(),
            ],
            ..lsp_types::ExecuteCommandOptions::default()
        }),
        ..ServerCapabilities::default()
    };
    let opened = connection
        .initialize(serde_json::to_value(capabilities).context("encoding capabilities")?)
        .context("initialize handshake")?;
    let analysis = Analysis::new().context("building analyzers from bundled rules")?;
    let mut documents: BTreeMap<String, String> = BTreeMap::new();
    let root = root_of(&opened);
    let mut held = Snapshot::new(root.clone());
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).context("shutdown")? {
                    return Ok(());
                }
                let answer = match request.method.as_str() {
                    "textDocument/hover" => hover::answer(&request, &held.derived(&documents)),
                    "textDocument/codeAction" => {
                        action::answer(&request, &held.derived(&documents), &analysis)
                    }
                    "textDocument/inlayHint" => hint::answer(&request, &held.derived(&documents)),
                    "workspace/executeCommand" => {
                        command::answer(&request, &held.derived(&documents))
                    }
                    _ => lsp_server::Response::new_err(
                        request.id.clone(),
                        lsp_server::ErrorCode::MethodNotFound as i32,
                        format!("method `{}` is not served", request.method)
                    )
                };
                connection
                    .sender
                    .send(Message::Response(answer))
                    .context("answering a request")?;
            }
            Message::Notification(notification) => {
                if let Some((uri, text)) = document_of(&notification) {
                    documents.insert(uri.as_str().to_owned(), text.clone());
                    held.touch();
                    let settled = notification.method != DidChangeTextDocument::METHOD;
                    let derived = settled.then(|| held.derived(&documents));
                    publish(
                        connection,
                        &analysis,
                        derived.as_ref(),
                        root.as_deref(),
                        uri,
                        &text
                    )?;
                }
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// The workspace root the client named at initialize, where it named
/// one: the first workspace folder, or the older root field.
fn root_of(opened: &serde_json::Value) -> Option<std::path::PathBuf> {
    opened["workspaceFolders"][0]["uri"]
        .as_str()
        .or_else(|| opened["rootUri"].as_str())
        .map(snapshot::path_of)
}

/// The document one notification carries, where it carries one whole.
fn document_of(notification: &Notification) -> Option<(Uri, String)> {
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let opened: DidOpenTextDocumentParams =
                serde_json::from_value(notification.params.clone()).ok()?;
            Some((opened.text_document.uri, opened.text_document.text))
        }
        DidChangeTextDocument::METHOD => {
            let changed: DidChangeTextDocumentParams =
                serde_json::from_value(notification.params.clone()).ok()?;
            let text = changed.content_changes.into_iter().next_back()?.text;
            Some((changed.text_document.uri, text))
        }
        DidSaveTextDocument::METHOD => {
            let saved: DidSaveTextDocumentParams =
                serde_json::from_value(notification.params.clone()).ok()?;
            Some((saved.text_document.uri, saved.text?))
        }
        _ => None
    }
}

/// Analyzes one document and publishes its diagnostics: the per-file
/// findings always, the workspace-derived intent and invariant
/// diagnostics where a settled snapshot is handed in.
fn publish(
    connection: &Connection,
    analysis: &Analysis,
    derived: Option<&snapshot::Derived<'_>>,
    root: Option<&std::path::Path>,
    uri: Uri,
    text: &str
) -> anyhow::Result<()> {
    let path = std::path::PathBuf::from(uri.path().as_str());
    let mut diagnostics = analysis.diagnostics(path.clone(), text);
    if let Some(held) = derived {
        diagnostics.extend(intent::diagnostics(held, &path));
        diagnostics.extend(invariant::diagnostics(held, root, &path));
    }
    let published = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None
    };
    connection
        .sender
        .send(Message::Notification(Notification::new(
            PublishDiagnostics::METHOD.to_owned(),
            published
        )))
        .context("publishing diagnostics")
}
