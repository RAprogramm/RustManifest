// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The rule explanation tool and the answer shape it returns.
//!
//! An explanation restates what the bundled pack declares about one rule
//! and resolves its rationale URI to the bundled methodology excerpt.

use rmcp::{ErrorData, Json, handler::server::wrapper::Parameters, tool, tool_router};
use rustmanifest_schema::{RuleDefinition, Severity};
use schemars::JsonSchema;
use serde::Serialize;

use super::{ReviewServer, catalog::find_rule, parameters::RuleIdParams};
use crate::resources;

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

#[tool_router(router = explanation_router, vis = "pub(super)")]
impl ReviewServer {
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;
    use crate::workspace::WorkspaceRoot;

    fn server_for(dir: &Path) -> ReviewServer {
        ReviewServer::new(WorkspaceRoot::new(dir).unwrap())
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
}
