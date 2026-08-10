// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tools that report what the engine knows about a workspace path.
//!
//! Each tool resolves the requested path inside the workspace root and
//! runs the deterministic knowledge computation on the blocking pool.
//! All messages are English regardless of request language.

use rmcp::{ErrorData, Json, handler::server::wrapper::Parameters, tool, tool_router};

use super::{
    ReviewServer,
    parameters::{FactsParams, OverviewParams, PathParams}
};

#[tool_router(router = knowledge_router, vis = "pub(super)")]
impl ReviewServer {
    /// Render everything the engine knows about a path as one Markdown
    /// report: size and coverage honesty, module map with cycle warnings,
    /// central types, behavioral roles, outside-world boundaries, most
    /// contended state, and learned conventions.
    #[tool(annotations(read_only_hint = true))]
    async fn project_overview(
        &self,
        Parameters(params): Parameters<OverviewParams>
    ) -> Result<String, ErrorData> {
        let path = self.resolved(&params.path)?;
        let top = params.top.unwrap_or(10);
        blocking(move || crate::knowledge::overview(&path, top)).await
    }

    /// Return behavioral facts of every function under a path as a JSON
    /// array: reads, writes, constructions, error types, effects,
    /// parameter ownership, and the derived behavioral role.
    #[tool(annotations(read_only_hint = true))]
    async fn function_facts(
        &self,
        Parameters(params): Parameters<FactsParams>
    ) -> Result<Json<super::answers::FactsAnswer>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::facts(&path, params.filter.as_deref()))
            .await
            .map(|facts| {
                Json(super::answers::FactsAnswer {
                    facts
                })
            })
    }

    /// List every inline suppression under a path with whether the full
    /// bundled-rules run used it: what is silenced, on what written
    /// ground, and which silences have gone stale.
    #[tool(annotations(read_only_hint = true))]
    async fn suppressions(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<super::answers::SuppressionsAnswer>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::suppressions(&path))
            .await
            .map(|suppressions| {
                Json(super::answers::SuppressionsAnswer {
                    suppressions
                })
            })
    }

    /// Learn per-role convention clauses over a path and return them as
    /// an auditable rule list; training is deterministic.
    #[tool(annotations(read_only_hint = true))]
    async fn learn_conventions(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<super::answers::ClausesAnswer>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::conventions(&path))
            .await
            .map(|clauses| {
                Json(super::answers::ClausesAnswer {
                    clauses
                })
            })
    }
}

/// Runs a synchronous knowledge computation on a blocking worker and maps
/// failures to MCP errors.
async fn blocking<T, F>(work: F) -> Result<T, ErrorData>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, rustmanifest_engine::EngineError> + Send + 'static
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| {
            ErrorData::internal_error(format!("knowledge task failed: {error}"), None)
        })?
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))
}
