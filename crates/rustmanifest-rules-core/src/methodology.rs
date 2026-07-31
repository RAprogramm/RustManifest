// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Methodology documents embedded into the server binary at compile time.
//!
//! Every document is exposed over MCP under a `rustmanifest://methodology/`
//! URI, so the binary is self-contained. Rule `rationale-uri` values point at
//! these documents, optionally with a `#fragment` selecting one section;
//! [`rationale_excerpt`] resolves such a URI to the referenced Markdown.

/// One methodology document bundled at compile time.
#[derive(Debug, Clone, Copy)]
pub struct MethodologyDoc {
    /// Canonical `rustmanifest://` URI of the document.
    pub uri: &'static str,
    /// Programmatic resource name.
    pub name: &'static str,
    /// Human-readable document title.
    pub title: &'static str,
    /// Repository path the content was embedded from.
    pub source_path: &'static str,
    /// Document content in Markdown (English).
    pub content: &'static str,
}

/// Every methodology document exposed by the server.
pub static DOCS: [MethodologyDoc; 9] = [
    MethodologyDoc {
        uri: "rustmanifest://methodology/structure",
        name: "structure",
        title: "Structural Design Principles",
        source_path: "STRUCTURE.md",
        content: include_str!("../../../STRUCTURE.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/code-quality",
        name: "code-quality",
        title: "Code Quality and Maintainability",
        source_path: "code-review-methodology/en/code-quality.md",
        content: include_str!("../../../code-review-methodology/en/code-quality.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/examples",
        name: "examples",
        title: "Real Project Examples",
        source_path: "code-review-methodology/en/examples.md",
        content: include_str!("../../../code-review-methodology/en/examples.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/index",
        name: "index",
        title: "Code Review Methodology Navigation",
        source_path: "code-review-methodology/en/INDEX.md",
        content: include_str!("../../../code-review-methodology/en/INDEX.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/performance-issues",
        name: "performance-issues",
        title: "Finding Performance Issues",
        source_path: "code-review-methodology/en/performance-issues.md",
        content: include_str!("../../../code-review-methodology/en/performance-issues.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/quick-reference",
        name: "quick-reference",
        title: "Quick Reference - Code Review Cheat Sheet",
        source_path: "code-review-methodology/en/quick-reference.md",
        content: include_str!("../../../code-review-methodology/en/quick-reference.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/readme",
        name: "readme",
        title: "Code Analysis and Vulnerability Detection Methodology",
        source_path: "code-review-methodology/en/README.md",
        content: include_str!("../../../code-review-methodology/en/README.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/rust-specific",
        name: "rust-specific",
        title: "Rust-Specific Issues and Patterns",
        source_path: "code-review-methodology/en/rust-specific.md",
        content: include_str!("../../../code-review-methodology/en/rust-specific.md"),
    },
    MethodologyDoc {
        uri: "rustmanifest://methodology/security-vulnerabilities",
        name: "security-vulnerabilities",
        title: "Finding Security Vulnerabilities",
        source_path: "code-review-methodology/en/security-vulnerabilities.md",
        content: include_str!("../../../code-review-methodology/en/security-vulnerabilities.md"),
    },
];

/// Looks up a document by its exact `rustmanifest://` URI (no fragment).
#[must_use]
pub fn find(uri: &str) -> Option<&'static MethodologyDoc> {
    DOCS.iter().find(|doc| doc.uri == uri)
}

/// Strips an optional `#fragment` from `uri` and returns the base URI.
#[must_use]
pub fn base_uri(uri: &str) -> &str {
    uri.split_once('#').map_or(uri, |(base, _)| base)
}

/// Resolves a rationale URI (base plus optional fragment) to the Markdown it
/// references.
///
/// When the fragment matches a section heading of the bundled document, only
/// that section is returned; otherwise the whole document is returned. `None`
/// means the base URI does not map to any bundled resource.
#[must_use]
pub fn rationale_excerpt(uri: &str) -> Option<String> {
    let (base, fragment) = uri
        .split_once('#')
        .map_or((uri, None), |(base, fragment)| (base, Some(fragment)));
    let doc = find(base)?;
    fragment
        .and_then(|fragment| section(doc.content, fragment))
        .or_else(|| Some(doc.content.to_owned()))
}

/// Extracts the Markdown section whose heading matches `fragment`.
///
/// A heading matches when its slug contains the fragment slug, so the
/// numbered heading `## 1. Entity Naming` matches the anchor
/// `entity-naming`. The section spans from the matched heading up to the
/// next heading of the same or a higher level.
fn section(content: &str, fragment: &str) -> Option<String> {
    let target = slug(fragment);
    if target.is_empty() {
        return None;
    }
    let mut collected: Vec<&str> = Vec::new();
    let mut matched_level: Option<usize> = None;
    for line in content.lines() {
        let heading = heading_level(line);
        if let Some(level) = matched_level {
            if heading.is_some_and(|current| current <= level) {
                break;
            }
            collected.push(line);
        } else if let Some(current) = heading
            && slug(line.trim_start_matches('#')).contains(&target)
        {
            matched_level = Some(current);
            collected.push(line);
        }
    }
    if collected.is_empty() {
        None
    } else {
        Some(collected.join("\n").trim_end().to_owned())
    }
}

/// Returns the ATX heading level of `line`, if it is a Markdown heading.
fn heading_level(line: &str) -> Option<usize> {
    let count = line
        .chars()
        .take_while(|&character| character == '#')
        .count();
    ((1..=6).contains(&count) && line.chars().nth(count) == Some(' ')).then_some(count)
}

/// Converts heading or anchor text into a lowercase dash-separated slug.
fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_dash = true;
    for character in text.trim().chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
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
    fn every_document_has_nonempty_content_and_unique_uri() {
        for doc in &DOCS {
            assert!(!doc.content.is_empty(), "empty content: {}", doc.uri);
            assert!(doc.uri.starts_with("rustmanifest://methodology/"));
        }
        let mut uris: Vec<&str> = DOCS.iter().map(|doc| doc.uri).collect();
        uris.sort_unstable();
        uris.dedup();
        assert_eq!(uris.len(), DOCS.len());
    }

    #[test]
    fn find_resolves_known_and_rejects_unknown() {
        assert!(find("rustmanifest://methodology/structure").is_some());
        assert!(find("rustmanifest://methodology/unknown").is_none());
    }

    #[test]
    fn rationale_excerpt_resolves_numbered_section_by_anchor() {
        let excerpt =
            rationale_excerpt("rustmanifest://methodology/structure#entity-naming").unwrap();
        assert!(excerpt.starts_with("## 1. Entity Naming"));
        assert!(!excerpt.contains("## 2. Method Naming"));
    }

    #[test]
    fn rationale_excerpt_falls_back_to_full_document() {
        let excerpt =
            rationale_excerpt("rustmanifest://methodology/structure#no-such-anchor").unwrap();
        assert!(excerpt.contains("# Structural Design Principles"));
    }

    #[test]
    fn rationale_excerpt_rejects_unknown_base() {
        assert!(rationale_excerpt("rustmanifest://methodology/unknown#x").is_none());
    }

    #[test]
    fn base_uri_strips_fragment() {
        assert_eq!(
            base_uri("rustmanifest://methodology/structure#entity-naming"),
            "rustmanifest://methodology/structure"
        );
        assert_eq!(
            base_uri("rustmanifest://methodology/structure"),
            "rustmanifest://methodology/structure"
        );
    }
}
