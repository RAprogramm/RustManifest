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
    parameters::{
        DeduceParams, FactsParams, KeptParams, OverviewParams, PathParams, WhenceParams
    }
};

#[tool_router(router = knowledge_router, vis = "pub(super)")]
impl ReviewServer {
    /// Render everything the engine knows about a path as one Markdown
    /// report: module map with cycle warnings, central types, behavioral roles,
    /// outside-world boundaries, most contended state, and learned conventions.
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

    /// Deduce the impl of a trait for a workspace type and answer with
    /// the chain itself: one recorded fact per step, the workspace's own
    /// precedent advising after the catalog's choice, the block
    /// installed in memory and confirmed by re-reading — no byte
    /// touches a disk.
    #[tool(annotations(read_only_hint = true))]
    async fn deduce_trait(
        &self,
        Parameters(params): Parameters<DeduceParams>
    ) -> Result<Json<crate::knowledge::DeductionRecord>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::deduction(&path, &params.type_name, &params.trait_name))
            .await
            .map(Json)
    }

    /// Derive every intent the written calls under a path prove: a call
    /// only an uninstalled ability can satisfy is the author's stated
    /// intent, and each record cites the function that wrote it as
    /// evidence.
    #[tool(annotations(read_only_hint = true))]
    async fn derived_intents(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<super::answers::IntentsAnswer>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::intents(&path))
            .await
            .map(|intents| {
                Json(super::answers::IntentsAnswer {
                    intents
                })
            })
    }

    /// Answer where the value a function hands back comes from: each
    /// written position cited with its byte range, the origin restated
    /// across calls, refusals naming candidates with their files.
    #[tool(annotations(read_only_hint = true))]
    async fn whence_value(
        &self,
        Parameters(params): Parameters<WhenceParams>
    ) -> Result<Json<crate::knowledge::AnswerRecord>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::whence(&path, &params.function))
            .await
            .map(Json)
    }

    /// Answer why the program keeps a store: the declaration that makes
    /// it a store, every verb proven at its own bytes, the routes
    /// serving it, and the writes that tell someone about it.
    #[tool(annotations(read_only_hint = true))]
    async fn why_kept(
        &self,
        Parameters(params): Parameters<KeptParams>
    ) -> Result<Json<crate::knowledge::AnswerRecord>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::kept(&path, &params.store))
            .await
            .map(Json)
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
    F: FnOnce() -> Result<T, masterror::AppError> + Send + 'static
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| {
            ErrorData::internal_error(format!("knowledge task failed: {error}"), None)
        })?
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))
}
