// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `dead-function` check: a private function no resolved or unresolved
//! call ever mentions.
//!
//! Reported only when every gate holds: the function is not `pub`, not a
//! test, not a trait-impl method, carries no attribute (an attribute can
//! mean an external entry point), is not `main` and not `_`-prefixed,
//! lives in a file reachable from a crate root (unreachable files are the
//! `orphan-file` diagnosis — the code there is disconnected, not dead),
//! and its bare name appears in **no** call candidate anywhere in the
//! workspace — resolved or not. An ambiguous or macro-hidden mention is
//! reason enough to stay silent.

use std::collections::BTreeSet;

use quote::ToTokens;
use rustmanifest_schema::TraceStep;
use syn::visit::Visit;

use super::{expr::token_words, insight::Insight, module_overview, workspace::Workspace};
use crate::source::Source;

/// Reports provably unreferenced private functions.
pub(super) fn insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let facts = workspace.facts();
    let graph = workspace.modules();
    let mut reachable = vec![false; sources.len()];
    for module in graph.modules() {
        if module.layout
            && let Some(flag) = reachable.get_mut(module.source)
        {
            *flag = true;
        }
    }
    let mentioned = mentioned_names(sources);
    let overview = module_overview::of(graph, sources);
    let module_of = |source: usize| -> String {
        sources
            .get(source)
            .and_then(|entry| {
                overview
                    .modules()
                    .iter()
                    .find(|module| module.file == entry.path())
            })
            .map_or_else(|| "?".to_owned(), |module| module.path.clone())
    };
    let mut found = Vec::new();
    for fact in facts {
        if fact.flags.public()
            || fact.flags.test()
            || fact.flags.trait_impl()
            || fact.flags.attributed()
        {
            continue;
        }
        let bare = fact.function.rsplit("::").next().unwrap_or(&fact.function);
        if bare == "main" || bare.starts_with('_') {
            continue;
        }
        if !reachable.get(fact.source).copied().unwrap_or(false) {
            continue;
        }
        if mentioned.contains(bare) {
            continue;
        }
        let module = module_of(fact.source);
        let detail = format!(
            "fn `{}` in module `{module}` is private and no call in the workspace mentions it; \
             remove it or wire it in",
            fact.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!(
                    "`{}` is private, unattributed, and not a test",
                    fact.function
                ),
                reason: "no path outside the workspace can call it".to_owned()
            },
            TraceStep {
                fact:   format!("nothing anywhere names `{bare}`"),
                reason: "calls, path values, imports, and macro tokens were all checked"
                    .to_owned()
            },
            TraceStep {
                fact:   "its file is reachable from a crate root".to_owned(),
                reason: "so this is dead code, not a forgotten `mod` declaration".to_owned()
            },
        ];
        found.push(Insight::new(fact.source, fact.range, detail, trace));
    }
    found
}

/// Collects every name the workspace mentions in any way: expression
/// paths (calls and bare values), `use`-tree leaves, and each identifier
/// word inside macro invocation tokens. A name in this set can never be
/// declared dead.
fn mentioned_names(sources: &[Source]) -> BTreeSet<String> {
    let mut scan = MentionScan {
        names: BTreeSet::new()
    };
    for source in sources {
        if let Some(file) = source.tree() {
            scan.visit_file(file);
        }
    }
    scan.names
}

/// Visitor accumulating mentioned names.
struct MentionScan {
    /// Every mentioned identifier.
    names: BTreeSet<String>
}

impl<'ast> Visit<'ast> for MentionScan {
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(segment) = path.path.segments.last() {
            self.names.insert(segment.ident.to_string());
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.names.insert(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_use_tree(&mut self, tree: &'ast syn::UseTree) {
        match tree {
            syn::UseTree::Name(name) => {
                self.names.insert(name.ident.to_string());
            }
            syn::UseTree::Rename(rename) => {
                self.names.insert(rename.ident.to_string());
            }
            _ => {}
        }
        syn::visit::visit_use_tree(self, tree);
    }

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if !attribute.path().is_ident("doc") {
            for word in token_words(&attribute.meta.to_token_stream().to_string()) {
                self.names.insert(word);
            }
        }
        syn::visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        for word in token_words(&invocation.tokens.to_string()) {
            self.names.insert(word);
        }
        syn::visit::visit_macro(self, invocation);
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
    use std::path::PathBuf;

    use super::insights;
    use crate::{semantic::workspace::Workspace, source::Source};

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    #[test]
    fn callback_and_macro_mentions_silence_the_check() {
        let sources = [source(
            "src/lib.rs",
            "pub fn run(values: &[u64]) -> Vec<u64> { values.iter().map(double).collect() }\nfn \
             double(v: &u64) -> u64 { v * 2 }\npub fn shout(name: &str) { println!(\"{}\", \
             greet(name)); }\nfn greet(name: &str) -> String { name.to_owned() }"
        )];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn unreferenced_private_function_is_reported() {
        let sources = [source(
            "src/lib.rs",
            "pub fn used() { helper(); }\nfn helper() {}\nfn stale() {}"
        )];
        let found = insights(&Workspace::new(&sources));
        assert_eq!(found.len(), 1);
        assert!(found[0].detail.contains("`stale`"));
    }

    #[test]
    fn called_and_public_functions_pass() {
        let sources = [source(
            "src/lib.rs",
            "pub fn used() { helper(); }\nfn helper() {}\npub fn api() {}"
        )];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn any_mention_by_name_silences_even_when_unresolved() {
        let sources = [source(
            "src/lib.rs",
            "struct A;\nimpl A { fn ping(&self) {} }\nstruct B;\nimpl B { fn ping(&self) {} \
             }\npub fn run(x: u32) { x.ping(); }"
        )];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn attributed_test_underscore_and_main_are_exempt() {
        let sources = [source(
            "src/main.rs",
            "fn main() {}\nfn _scratch() {}\n#[test]\nfn probe() {}\n#[inline]\nfn pinned() {}"
        )];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn functions_in_unreachable_files_are_the_orphan_diagnosis() {
        let sources = [
            source("src/lib.rs", "pub fn api() {}"),
            source("src/lost.rs", "fn maybe_needed() {}")
        ];
        assert!(insights(&Workspace::new(&sources)).is_empty());
    }
}
