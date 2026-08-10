// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! MCP server surface: typed review tools and methodology resources.
//!
//! Tools run the bundled rules pack over files confined to the workspace
//! root; resources expose the embedded methodology documents under
//! `rustmanifest://methodology/` URIs. All server-facing strings are
//! English regardless of request language.

mod answers;
mod catalog;
mod corpus;
mod explanation;
mod knowledge;
mod methodology;
mod parameters;
mod review;

use std::path::PathBuf;

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{
        Implementation, ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResponse, ServerCapabilities, ServerInfo
    },
    service::RequestContext,
    tool_handler
};

pub use self::{
    explanation::RuleExplanation,
    parameters::{
        DeduceParams, FactsParams, OverviewParams, PathParams, RuleIdParams, RulePathParams
    }
};
use crate::workspace::WorkspaceRoot;

/// MCP server exposing the `rustmanifest` engine over stdio.
#[derive(Debug, Clone)]
pub struct ReviewServer {
    root: WorkspaceRoot
}

impl ReviewServer {
    /// Creates a server confined to the given workspace root.
    #[must_use]
    pub const fn new(root: WorkspaceRoot) -> Self {
        Self {
            root
        }
    }

    /// Creates a server rooted at the process current directory.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] when the current directory is
    /// unavailable or cannot be canonicalized.
    pub fn from_current_dir() -> std::io::Result<Self> {
        Ok(Self::new(WorkspaceRoot::from_current_dir()?))
    }

    /// Whole tool surface of the server: rule execution, knowledge
    /// reports, and rule explanation.
    fn tool_router() -> ToolRouter<Self> {
        Self::review_router() + Self::knowledge_router() + Self::explanation_router()
    }

    /// Resolves a client path inside the workspace root or returns an
    /// invalid-params error with an English message.
    fn resolved(&self, requested: &str) -> Result<PathBuf, ErrorData> {
        self.root
            .resolve(requested)
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
    }
}

#[tool_handler]
impl ServerHandler for ReviewServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build()
        )
        .with_server_info(Implementation::new(
            "rust-manifest",
            env!("CARGO_PKG_VERSION")
        ))
        .with_instructions(
            "rustmanifest review engine. Tools run the bundled methodology rules over files \
             inside the workspace root (the server's startup directory); paths outside the root \
             are rejected. Resources expose the methodology documents under \
             rustmanifest://methodology/ URIs; finding rationale URIs resolve to them."
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut listed = methodology::listing();
        listed.resources.extend(corpus::listing(self.root.path()));
        Ok(listed)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>
    ) -> Result<ReadResourceResponse, ErrorData> {
        if let Some(answered) = corpus::read(self.root.path(), &request.uri) {
            return answered;
        }
        methodology::read(&request.uri)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    #[test]
    fn tool_router_exposes_all_eleven_tools() {
        let tools = ReviewServer::tool_router().list_all();
        let mut names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "check_rule",
                "deduce_trait",
                "derived_intents",
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
}
