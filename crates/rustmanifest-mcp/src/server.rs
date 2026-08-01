// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! MCP server surface: typed review tools and methodology resources.
//!
//! Tools run the bundled rules pack over files confined to the workspace
//! root; resources expose the embedded methodology documents under
//! `rustmanifest://methodology/` URIs. All server-facing strings are
//! English regardless of request language.

use std::path::PathBuf;

use rmcp::{
    ErrorData, Json, RoleServer, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{
        Implementation, ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
        ServerInfo
    },
    service::RequestContext,
    tool, tool_handler, tool_router
};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, Rule, RuleDefinition, Severity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{resources, workspace::WorkspaceRoot};

/// Identifier prefix of the structural rules family (`STRUCTURE.md`).
const STRUCT_RULE_PREFIX: &str = "RM-STRUCT-";

/// Parameters for tools that accept a single workspace path.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathParams {
    /// File or directory path resolved against the workspace root.
    pub path: String
}

/// Parameters for `project_overview`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OverviewParams {
    /// File or directory path resolved against the workspace root.
    pub path: String,
    /// Cap for every ranked list in the report; defaults to 10.
    pub top:  Option<usize>
}

/// Parameters for `function_facts`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactsParams {
    /// File or directory path resolved against the workspace root.
    pub path:   String,
    /// Only include functions whose qualified name contains this
    /// substring.
    pub filter: Option<String>
}

/// Parameters for `explain_rule`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RuleIdParams {
    /// Rule identifier, e.g. `RM-STRUCT-001`.
    pub rule_id: String
}

/// Parameters for `check_rule`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RulePathParams {
    /// Rule identifier, e.g. `RM-SEC-001`.
    pub rule_id: String,
    /// File or directory path resolved against the workspace root.
    pub path:    String
}

/// Structured answer returned by `explain_rule`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RuleExplanation {
    /// Stable rule identifier.
    pub id:                  String,
    /// Short human-readable title.
    pub title:               String,
    /// Default severity declared by the rules pack.
    pub severity:            Severity,
    /// Analysis tier: `pattern`, `ast`, or `semantic`.
    pub tier:                String,
    /// `rustmanifest://` URI of the methodology rationale.
    pub rationale_uri:       String,
    /// Methodology excerpt the rationale URI resolves to, when bundled.
    pub methodology_excerpt: Option<String>
}

/// MCP server exposing the `rustmanifest` engine over stdio.
#[derive(Debug, Clone)]
pub struct ReviewServer {
    root: WorkspaceRoot
}

#[tool_router]
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

    /// Run every bundled rule against one file inside the workspace root and
    /// return the findings as a JSON array.
    #[tool(annotations(read_only_hint = true))]
    async fn review_file(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<Vec<Finding>>, ErrorData> {
        let path = self.resolved(&params.path)?;
        if !path.is_file() {
            return Err(ErrorData::invalid_params(
                format!("path `{}` is not a file", params.path),
                None
            ));
        }
        findings_for(RULES.clone(), path).await.map(Json)
    }

    /// Run every bundled rule against a file or directory inside the
    /// workspace root and return the findings as a JSON array.
    #[tool(annotations(read_only_hint = true))]
    async fn review_path(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<Vec<Finding>>, ErrorData> {
        let path = self.resolved(&params.path)?;
        findings_for(RULES.clone(), path).await.map(Json)
    }

    /// Run exactly one bundled rule against a file or directory inside the
    /// workspace root and return the findings as a JSON array.
    #[tool(annotations(read_only_hint = true))]
    async fn check_rule(
        &self,
        Parameters(params): Parameters<RulePathParams>
    ) -> Result<Json<Vec<Finding>>, ErrorData> {
        let rule = find_rule(&params.rule_id)?;
        let path = self.resolved(&params.path)?;
        findings_for(vec![rule.clone()], path).await.map(Json)
    }

    /// Run only the structural rules (`RM-STRUCT-*`) against a file or
    /// directory inside the workspace root and return the findings as a
    /// JSON array.
    #[tool(annotations(read_only_hint = true))]
    async fn lint_structure(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<Vec<Finding>>, ErrorData> {
        let path = self.resolved(&params.path)?;
        let rules: Vec<Rule> = RULES
            .iter()
            .filter(|rule| rule.id.starts_with(STRUCT_RULE_PREFIX))
            .cloned()
            .collect();
        findings_for(rules, path).await.map(Json)
    }

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
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::facts(&path, params.filter.as_deref()))
            .await
            .map(Json)
    }

    /// Learn per-role convention clauses over a path and return them as
    /// an auditable rule list; training is deterministic.
    #[tool(annotations(read_only_hint = true))]
    async fn learn_conventions(
        &self,
        Parameters(params): Parameters<PathParams>
    ) -> Result<Json<Vec<crate::knowledge::ConventionClause>>, ErrorData> {
        let path = self.resolved(&params.path)?;
        blocking(move || crate::knowledge::conventions(&path))
            .await
            .map(Json)
    }

    /// Explain one bundled rule: identifier, title, severity, tier,
    /// rationale URI, and the methodology excerpt the URI resolves to.
    #[tool(annotations(read_only_hint = true))]
    #[expect(
        clippy::unused_self,
        reason = "the tool router dispatches every tool through a service receiver"
    )]
    fn explain_rule(
        &self,
        Parameters(params): Parameters<RuleIdParams>
    ) -> Result<Json<RuleExplanation>, ErrorData> {
        let rule = find_rule(&params.rule_id)?;
        Ok(Json(RuleExplanation {
            id:                  rule.id.clone(),
            title:               rule.title.clone(),
            severity:            rule.severity,
            tier:                tier_name(&rule.definition).to_owned(),
            rationale_uri:       rule.rationale_uri.clone(),
            methodology_excerpt: resources::rationale_excerpt(&rule.rationale_uri)
        }))
    }

    /// Resolves a client path inside the workspace root or returns an
    /// invalid-params error with an English message.
    fn resolved(&self, requested: &str) -> Result<PathBuf, ErrorData> {
        self.root
            .resolve(requested)
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
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

/// Runs the given rules on a blocking worker and maps failures to MCP
/// errors.
async fn findings_for(rules: Vec<Rule>, path: PathBuf) -> Result<Vec<Finding>, ErrorData> {
    tokio::task::spawn_blocking(move || analysis_run(&rules, &path))
        .await
        .map_err(|error| {
            ErrorData::internal_error(format!("analysis task failed: {error}"), None)
        })?
}

/// Synchronous analysis body executed on the blocking pool.
fn analysis_run(rules: &[Rule], path: &std::path::Path) -> Result<Vec<Finding>, ErrorData> {
    crate::analysis::run_rules(rules, path)
        .map_err(|error| ErrorData::internal_error(format!("analysis failed: {error}"), None))
}

/// Looks up a bundled rule by identifier.
fn find_rule(rule_id: &str) -> Result<&'static Rule, ErrorData> {
    RULES.iter().find(|rule| rule.id == rule_id).ok_or_else(|| {
        let known: Vec<&str> = RULES.iter().map(|rule| rule.id.as_str()).collect();
        ErrorData::invalid_params(
            format!(
                "unknown rule id `{rule_id}`; known rule ids: {}",
                known.join(", ")
            ),
            None
        )
    })
}

/// Returns the analysis tier name of a rule definition.
const fn tier_name(definition: &RuleDefinition) -> &'static str {
    match definition {
        RuleDefinition::Pattern {
            ..
        } => "pattern",
        RuleDefinition::Ast {
            ..
        } => "ast",
        RuleDefinition::Semantic {
            ..
        } => "semantic"
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
            env!("CARGO_PKG_NAME"),
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
        let listed = resources::DOCS
            .iter()
            .map(|doc| {
                Resource::new(doc.uri, doc.name)
                    .with_title(doc.title)
                    .with_description(format!("Embedded from `{}`.", doc.source_path))
                    .with_mime_type("text/markdown")
            })
            .collect();
        Ok(ListResourcesResult::with_all_items(listed))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>
    ) -> Result<ReadResourceResponse, ErrorData> {
        resources::find(resources::base_uri(&request.uri)).map_or_else(
            || {
                Err(ErrorData::resource_not_found(
                    format!("unknown resource URI `{}`", request.uri),
                    None
                ))
            },
            |doc| {
                Ok(ReadResourceResult::new(vec![
                    ResourceContents::text(doc.content, doc.uri).with_mime_type("text/markdown"),
                ])
                .into())
            }
        )
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
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::*;

    fn server_for(dir: &Path) -> ReviewServer {
        ReviewServer::new(WorkspaceRoot::new(dir).unwrap())
    }

    #[tokio::test]
    async fn review_file_reports_known_violation() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("bad.rs"),
            "fn main() { let password = \"hunter2\"; }\n"
        )
        .unwrap();
        let server = server_for(dir.path());
        let Json(findings) = server
            .review_file(Parameters(PathParams {
                path: "bad.rs".to_owned()
            }))
            .await
            .unwrap();
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule_id == "RM-SEC-001")
        );
    }

    #[tokio::test]
    async fn review_file_rejects_directory() {
        let dir = tempdir().unwrap();
        let server = server_for(dir.path());
        let error = server
            .review_file(Parameters(PathParams {
                path: ".".to_owned()
            }))
            .await
            .map(|Json(findings)| findings)
            .unwrap_err();
        assert!(error.message.contains("is not a file"));
    }

    #[tokio::test]
    async fn review_file_rejects_escaping_path() {
        let outer = tempdir().unwrap();
        fs::write(outer.path().join("secret.rs"), "fn main() {}\n").unwrap();
        let inner = outer.path().join("workspace");
        fs::create_dir(&inner).unwrap();
        let server = server_for(&inner);
        let error = server
            .review_file(Parameters(PathParams {
                path: "../secret.rs".to_owned()
            }))
            .await
            .map(|Json(findings)| findings)
            .unwrap_err();
        assert!(error.message.contains("escapes workspace root"));
    }

    #[tokio::test]
    async fn review_path_walks_directories() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("naming.rs"), "pub struct ConfigLoader;\n").unwrap();
        let server = server_for(dir.path());
        let Json(findings) = server
            .review_path(Parameters(PathParams {
                path: ".".to_owned()
            }))
            .await
            .unwrap();
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule_id == "RM-STRUCT-001")
        );
    }

    #[tokio::test]
    async fn lint_structure_reports_only_struct_rules() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("mixed.rs"),
            "pub struct ConfigLoader;\nfn main() { let password = \"hunter2\"; }\n"
        )
        .unwrap();
        let server = server_for(dir.path());
        let Json(findings) = server
            .lint_structure(Parameters(PathParams {
                path: ".".to_owned()
            }))
            .await
            .unwrap();
        assert!(!findings.is_empty());
        assert!(
            findings
                .iter()
                .all(|finding| finding.rule_id.starts_with(STRUCT_RULE_PREFIX))
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule_id == "RM-STRUCT-001")
        );
    }

    #[tokio::test]
    async fn check_rule_runs_exactly_one_rule() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("mixed.rs"),
            "pub struct ConfigLoader;\nfn main() { let password = \"hunter2\"; }\n"
        )
        .unwrap();
        let server = server_for(dir.path());
        let Json(findings) = server
            .check_rule(Parameters(RulePathParams {
                rule_id: "RM-STRUCT-001".to_owned(),
                path:    ".".to_owned()
            }))
            .await
            .unwrap();
        assert!(!findings.is_empty());
        assert!(
            findings
                .iter()
                .all(|finding| finding.rule_id == "RM-STRUCT-001")
        );
    }

    #[tokio::test]
    async fn check_rule_unknown_id_is_descriptive() {
        let dir = tempdir().unwrap();
        let server = server_for(dir.path());
        let error = server
            .check_rule(Parameters(RulePathParams {
                rule_id: "RM-NOPE-999".to_owned(),
                path:    ".".to_owned()
            }))
            .await
            .map(|Json(findings)| findings)
            .unwrap_err();
        assert!(error.message.contains("unknown rule id `RM-NOPE-999`"));
        assert!(error.message.contains("RM-STRUCT-001"));
    }

    #[test]
    fn explain_rule_resolves_known_id() {
        let dir = tempdir().unwrap();
        let server = server_for(dir.path());
        let Json(explanation) = server
            .explain_rule(Parameters(RuleIdParams {
                rule_id: "RM-STRUCT-001".to_owned()
            }))
            .unwrap();
        assert_eq!(explanation.id, "RM-STRUCT-001");
        assert_eq!(explanation.tier, "ast");
        assert_eq!(
            explanation.rationale_uri,
            "rustmanifest://methodology/structure#entity-naming"
        );
        assert!(
            explanation
                .methodology_excerpt
                .is_some_and(|excerpt| excerpt.contains("Entity Naming"))
        );
    }

    #[test]
    fn explain_rule_rejects_unknown_id() {
        let dir = tempdir().unwrap();
        let server = server_for(dir.path());
        let error = server
            .explain_rule(Parameters(RuleIdParams {
                rule_id: "RM-XXX-000".to_owned()
            }))
            .map(|Json(explanation)| explanation)
            .unwrap_err();
        assert!(error.message.contains("unknown rule id `RM-XXX-000`"));
    }

    #[test]
    fn every_rationale_uri_base_resolves_to_bundled_resource() {
        for rule in RULES.iter() {
            let base = resources::base_uri(&rule.rationale_uri);
            assert!(
                resources::find(base).is_some(),
                "rationale URI of {} does not resolve: {}",
                rule.id,
                rule.rationale_uri
            );
        }
    }

    #[test]
    fn tool_router_exposes_all_eight_tools() {
        let tools = ReviewServer::tool_router().list_all();
        let mut names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "check_rule",
                "explain_rule",
                "function_facts",
                "learn_conventions",
                "lint_structure",
                "project_overview",
                "review_file",
                "review_path"
            ]
        );
    }
}
