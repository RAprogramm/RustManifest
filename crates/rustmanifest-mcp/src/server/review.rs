// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tools that run bundled rules over a workspace path.
//!
//! Each tool resolves the requested path inside the workspace root, runs
//! the selected rules on the blocking pool, and returns the findings as a
//! JSON array. All messages are English regardless of request language.

use std::path::{Path, PathBuf};

use rmcp::{ErrorData, Json, handler::server::wrapper::Parameters, tool, tool_router};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, Rule};

use super::{
    ReviewServer,
    catalog::find_rule,
    parameters::{PathParams, RulePathParams}
};

/// Identifier prefix of the structural rules family (`STRUCTURE.md`).
const STRUCT_RULE_PREFIX: &str = "RM-STRUCT-";

#[tool_router(router = review_router, vis = "pub(super)")]
impl ReviewServer {
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
fn analysis_run(rules: &[Rule], path: &Path) -> Result<Vec<Finding>, ErrorData> {
    crate::analysis::run_rules(rules, path)
        .map_err(|error| ErrorData::internal_error(format!("analysis failed: {error}"), None))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::workspace::WorkspaceRoot;

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
}
