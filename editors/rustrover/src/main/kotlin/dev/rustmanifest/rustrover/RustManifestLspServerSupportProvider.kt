// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

package dev.rustmanifest.rustrover

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor

/**
 * Starts the rust-manifest server for Rust files. The IDE's own Rust
 * support keeps answering first; this server adds the fact cards,
 * ownership stories, ghost-text ownership marks, and the extraction
 * and implement actions on top.
 */
class RustManifestLspServerSupportProvider : LspServerSupportProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerSupportProvider.LspServerStarter
    ) {
        if (file.extension == "rs") {
            serverStarter.ensureServerStarted(RustManifestLspServerDescriptor(project))
        }
    }
}

/**
 * One project-wide server over stdio: the binary installed by
 * `cargo install rustmanifest-lsp`.
 */
private class RustManifestLspServerDescriptor(project: Project) :
    ProjectWideLspServerDescriptor(project, "Rust Manifest") {
    override fun isSupportedFile(file: VirtualFile): Boolean = file.extension == "rs"

    override fun createCommandLine(): GeneralCommandLine = GeneralCommandLine("rust-manifest")
}
