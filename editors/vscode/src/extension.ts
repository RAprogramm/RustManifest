// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

// The extension host side of rust-manifest: starts the binary over
// stdio for Rust documents and hands the whole protocol to the
// language client. The server runs beside rust-analyzer; the editor
// shows both hovers and merges both action lists, so nothing here
// touches or configures the other server.

import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    const settings = vscode.workspace.getConfiguration('rust-manifest');
    if (!settings.get<boolean>('enable', true)) {
        return;
    }
    const command = settings.get<string>('path', 'rust-manifest');
    const serverOptions: ServerOptions = {
        command,
        args: []
    };
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'rust' }]
    };
    client = new LanguageClient(
        'rust-manifest',
        'Rust Manifest',
        serverOptions,
        clientOptions
    );
    context.subscriptions.push({ dispose: () => void client?.stop() });
    try {
        await client.start();
    } catch {
        void vscode.window.showWarningMessage(
            'rust-manifest failed to start; install it with: cargo install rustmanifest-lsp'
        );
    }
}

export async function deactivate(): Promise<void> {
    await client?.stop();
    client = undefined;
}
