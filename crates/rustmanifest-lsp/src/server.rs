// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The protocol loop: initialize, then diagnostics on every open,
//! change and save.
//!
//! Full-text synchronization is deliberate for the first shipped
//! surface: the engine analyzes a whole file in around a millisecond,
//! so incremental edits would buy latency nobody is waiting on at the
//! price of a reconciliation the protocol is famous for getting wrong.

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

mod analysis;
mod position;

use self::analysis::Analysis;

/// Runs the server over `connection` until the client asks to shut down.
///
/// # Errors
///
/// Returns protocol failures: a broken transport, or a handshake the
/// client abandoned.
pub fn serve(connection: &Connection) -> anyhow::Result<()> {
    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..ServerCapabilities::default()
    };
    connection
        .initialize(serde_json::to_value(capabilities).context("encoding capabilities")?)
        .context("initialize handshake")?;
    let analysis = Analysis::new().context("building analyzers from bundled rules")?;
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).context("shutdown")? {
                    return Ok(());
                }
            }
            Message::Notification(notification) => {
                if let Some((uri, text)) = document_of(&notification) {
                    publish(connection, &analysis, uri, &text)?;
                }
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
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

/// Analyzes one document and publishes its diagnostics.
fn publish(
    connection: &Connection,
    analysis: &Analysis,
    uri: Uri,
    text: &str
) -> anyhow::Result<()> {
    let path = std::path::PathBuf::from(uri.path().as_str());
    let diagnostics = analysis.diagnostics(path, text);
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
