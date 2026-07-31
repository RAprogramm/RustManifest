// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `entity-naming` check: structures represent entities, not actions
//! (`STRUCTURE.md` §1).

use super::{Occurrence, flattened};

/// Agent suffixes that indicate a type named for what it does.
const AGENT_SUFFIXES: [&str; 8] = [
    "Loader",
    "Parser",
    "Handler",
    "Validator",
    "Manager",
    "Dispatcher",
    "Reader",
    "Generator",
];

/// Reports type definitions whose names carry an agent suffix.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        let (kind, ident) = match item {
            syn::Item::Struct(definition) => ("struct", &definition.ident),
            syn::Item::Enum(definition) => ("enum", &definition.ident),
            syn::Item::Trait(definition) => ("trait", &definition.ident),
            _ => continue,
        };
        let name = ident.to_string();
        if let Some(suffix) = agent_suffix(&name) {
            found.push(Occurrence::new(
                &ident.span(),
                format!(
                    "{kind} `{name}` is named for what it does (`-{suffix}`); name the entity \
                     for what it is"
                ),
            ));
        }
    }
    found
}

/// Returns the offending suffix when `name` ends with one, excluding names
/// that consist of the suffix alone (`Parser` inside a parsing library is a
/// domain entity in its own right).
fn agent_suffix(name: &str) -> Option<&'static str> {
    AGENT_SUFFIXES
        .into_iter()
        .find(|suffix| name.ends_with(suffix) && name.len() > suffix.len())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::occurrences;

    fn count(source: &str) -> usize {
        occurrences(&syn::parse_file(source).unwrap()).len()
    }

    #[test]
    fn agent_suffixed_types_are_reported() {
        assert_eq!(count("struct ConfigLoader;"), 1);
        assert_eq!(count("enum EventDispatcher {}"), 1);
        assert_eq!(count("trait RequestHandler {}"), 1);
    }

    #[test]
    fn entity_names_pass() {
        assert_eq!(count("struct Config; enum Event {} trait Request {}"), 0);
    }

    #[test]
    fn established_patterns_pass() {
        assert_eq!(count("struct RequestBuilder; trait TreeVisitor {}"), 0);
    }

    #[test]
    fn bare_suffix_word_passes() {
        assert_eq!(count("struct Parser; struct Reader;"), 0);
    }

    #[test]
    fn nested_module_types_are_reported() {
        assert_eq!(count("mod api { pub struct TokenGenerator; }"), 1);
    }
}
