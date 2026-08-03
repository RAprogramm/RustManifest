// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Lookup of bundled rules by identifier.
//!
//! An unknown identifier is rejected with an invalid-params error that
//! lists every identifier the bundled pack does carry.

use rmcp::ErrorData;
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::Rule;

/// Looks up a bundled rule by identifier.
pub(super) fn find_rule(rule_id: &str) -> Result<&'static Rule, ErrorData> {
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
