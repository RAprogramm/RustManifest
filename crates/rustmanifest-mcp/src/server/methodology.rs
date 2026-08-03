// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The methodology resource surface of the server.
//!
//! Documents are embedded at compile time and served under
//! `rustmanifest://methodology/` URIs as Markdown; finding rationale URIs
//! resolve to the same documents.

use rmcp::{
    ErrorData,
    model::{
        ListResourcesResult, ReadResourceResponse, ReadResourceResult, Resource, ResourceContents
    }
};

use crate::resources;

/// Lists every embedded methodology document as an MCP resource.
pub(super) fn listing() -> ListResourcesResult {
    let listed = resources::DOCS
        .iter()
        .map(|doc| {
            Resource::new(doc.uri, doc.name)
                .with_title(doc.title)
                .with_description(format!("Embedded from `{}`.", doc.source_path))
                .with_mime_type("text/markdown")
        })
        .collect();
    ListResourcesResult::with_all_items(listed)
}

/// Reads the methodology document a requested URI resolves to.
pub(super) fn read(uri: &str) -> Result<ReadResourceResponse, ErrorData> {
    resources::find(resources::base_uri(uri)).map_or_else(
        || {
            Err(ErrorData::resource_not_found(
                format!("unknown resource URI `{uri}`"),
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use rustmanifest_rules_core::RULES;

    use super::*;

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
}
