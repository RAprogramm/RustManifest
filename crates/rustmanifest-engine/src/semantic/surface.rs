// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! External surface recovery: the routes a service exposes, read from the
//! framework's own registration calls.
//!
//! A web framework declares its entry points as typed syntax — a router
//! value, a registration method taking a path literal and a handler
//! wrapped in an HTTP-method call, and composition methods folding one
//! router into another. This module walks those chains and produces the
//! route table: method, path, handler, and the byte range of the
//! registration that proved it.
//!
//! The vocabulary lives in [`FrameworkModel`] as data, so a second
//! framework is a table rather than a code path. Nothing is inferred and
//! nothing is dropped in silence: a source that does not parse, a path
//! that is not a literal, a handler that is not a method-router, a
//! `route`/`nest`/`merge` on a receiver that was never proven to be a
//! router, a composition naming no provider or naming several, a mount
//! prefix that is not a literal, and a composition cycle that leaves no
//! root each land in a counter or a named list on [`Surface`].
//!
//! Provider functions are keyed by their full path — the module path the
//! file implies, plus any enclosing module, impl or host function, plus
//! the item's own name — so two `routes()` in two modules stay two
//! providers. A composition argument is resolved against those keys by
//! suffix, preferring the nearest module; a tie is reported as ambiguous
//! rather than picked.

mod argument;
mod attribute;
mod binding;
mod expansion;
mod framework;
mod path;
mod provider;
mod registration;
mod resolution;
mod table;
mod unit;
mod walk;

use std::collections::{BTreeMap, BTreeSet};

use syn::Item;

pub(super) use self::framework::takes_from_path;
use self::{
    expansion::Expansion,
    path::module_path,
    provider::{Provider, collect},
    resolution::resolved_folds,
    table::conflicts,
    unit::units
};
pub use self::{
    framework::FrameworkModel,
    table::{Route, Surface}
};
use crate::source::Source;

/// Recovers the route table of `sources`.
///
/// Provider functions — those whose body registers or composes routes —
/// are collected first and keyed by full path, then their composition
/// arguments are resolved against those keys, then the table is expanded
/// from every root: a provider no other provider folds in. Composition
/// cycles are cut on revisit, so a mutually nesting pair yields each
/// route once instead of looping.
#[must_use]
pub fn surface(sources: &[Source]) -> Surface {
    let mut providers: BTreeMap<String, Provider> = BTreeMap::new();
    let mut recovered = Surface::default();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            recovered.unparsed += 1;
            continue;
        };
        let scope = module_path(source.path());
        let items: Vec<&Item> = file.items.iter().collect();
        let mut collected = Vec::new();
        units(&items, &scope, false, &mut collected);
        for unit in &collected {
            collect(unit, index, &mut providers, &mut recovered);
        }
    }
    let folds = resolved_folds(&providers, &mut recovered);
    let folded: BTreeSet<&str> = folds
        .values()
        .flatten()
        .map(|fold| fold.target.as_str())
        .collect();
    recovered.providers = providers.len();
    recovered.composed = folded.len();
    let roots: Vec<&str> = providers
        .keys()
        .map(String::as_str)
        .filter(|name| !folded.contains(name))
        .collect();
    if roots.is_empty() {
        if !providers.is_empty() {
            recovered.entangled = providers.keys().cloned().collect();
            recovered.discarded = providers
                .values()
                .map(|provider| provider.routes.len())
                .sum();
        }
        return recovered;
    }
    let mut expansion = Expansion {
        providers: &providers,
        folds:     &folds,
        seen:      BTreeSet::new(),
        routes:    Vec::new(),
        unplaced:  0
    };
    for root in roots {
        expansion.expand(root, "", false);
    }
    let mut routes = expansion.routes;
    recovered.unplaced = expansion.unplaced;
    routes.sort_by(|left, right| {
        (&left.path, &left.method, &left.handler).cmp(&(
            &right.path,
            &right.method,
            &right.handler
        ))
    });
    routes.dedup();
    recovered.conflicts = conflicts(&routes);
    recovered.routes = routes;
    recovered
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

    use super::surface;
    use crate::source::Source;

    fn source(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    fn paths(recovered: &super::Surface) -> Vec<&str> {
        recovered
            .routes
            .iter()
            .map(|route| route.path.as_str())
            .collect()
    }

    #[test]
    fn registrations_recover_method_path_and_handler() {
        let recovered = surface(&source(
            "pub fn routes() -> Router {\n    Router::new()\n        \
             .route(\"/api/events\", post(track_event))\n        .route(\"/api/events\", \
             get(list_events))\n}"
        ));
        let rows: Vec<(&str, &str, &str)> = recovered
            .routes
            .iter()
            .map(|route| {
                (
                    route.method.as_str(),
                    route.path.as_str(),
                    route.handler.as_str()
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("GET", "/api/events", "list_events"),
                ("POST", "/api/events", "track_event")
            ]
        );
        assert_eq!(recovered.dynamic, 0);
    }

    /// A method-router folded into another with `merge` carries verbs of
    /// its own. Reading only the outermost chain lost every one of them —
    /// three real endpoints of one corpus workspace were missing from the
    /// recovered table for exactly this reason.
    #[test]
    fn verbs_folded_into_a_method_router_are_recovered() {
        let recovered = surface(&source(
            "pub fn routes() -> Router {\n    Router::new().route(\"/api/me\", \
             get(read).merge(put(write).route_layer(guard())))\n}"
        ));
        let rows: Vec<(&str, &str, &str)> = recovered
            .routes
            .iter()
            .map(|route| {
                (
                    route.method.as_str(),
                    route.path.as_str(),
                    route.handler.as_str()
                )
            })
            .collect();
        assert_eq!(
            rows,
            [("GET", "/api/me", "read"), ("PUT", "/api/me", "write")]
        );
    }

    #[test]
    fn a_chain_behind_a_fold_keeps_every_verb_it_writes() {
        let recovered = surface(&source(
            "pub fn routes() -> Router {\n    Router::new().route(\"/api/x/{id}\", \
             get(read).merge(patch(update).delete(drop_it).route_layer(guard())))\n}"
        ));
        let mut methods: Vec<&str> = recovered
            .routes
            .iter()
            .map(|route| route.method.as_str())
            .collect();
        methods.sort_unstable();
        assert_eq!(methods, ["DELETE", "GET", "PATCH"]);
    }

    #[test]
    fn nesting_composes_prefixes_and_merging_keeps_paths() {
        let recovered = surface(&source(
            "fn inner() -> Router { Router::new().route(\"/list\", get(list)) }\nfn flat() -> \
             Router { Router::new().route(\"/health\", get(health)) }\nfn app() -> Router { \
             Router::new().nest(\"/api/v1\", inner()).merge(flat()) }"
        ));
        assert_eq!(paths(&recovered), ["/api/v1/list", "/health"]);
        assert_eq!(recovered.providers, 3);
        assert_eq!(recovered.composed, 2);
    }

    #[test]
    fn composed_counts_providers_folded_into_others() {
        let recovered = surface(&source(
            "fn inner() -> Router { Router::new().route(\"/list\", get(list)) }\nfn app() -> \
             Router { Router::new().nest(\"/api\", inner()) }"
        ));
        assert_eq!(recovered.providers, 2);
        assert_eq!(recovered.composed, 1);
        assert_eq!(recovered.providers - recovered.composed, 1);
    }

    #[test]
    fn a_registration_under_a_branch_is_recovered_and_marked_conditional() {
        let recovered = surface(&source(
            "fn routes(debug: bool) -> Router {\n    let mut app = Router::new().route(\"/live\", \
             get(live));\n    if debug {\n        app = app.route(\"/debug\", get(debug_handler));\n \
             }\n    app\n}"
        ));
        let rows: Vec<(&str, bool)> = recovered
            .routes
            .iter()
            .map(|route| (route.path.as_str(), route.conditional))
            .collect();
        assert_eq!(rows, [("/debug", true), ("/live", false)]);
        assert_eq!(recovered.conditional, 1);
    }

    #[test]
    fn registrations_inside_a_match_arm_or_a_closure_are_not_lost() {
        let matched = surface(&source(
            "fn routes(mode: Mode) -> Router {\n    match mode {\n        Mode::Full => \
             Router::new().route(\"/full\", get(full)),\n        Mode::Slim => \
             Router::new().route(\"/slim\", get(slim))\n    }\n}"
        ));
        assert_eq!(paths(&matched), ["/full", "/slim"]);
        assert_eq!(matched.conditional, 2);

        let closured = surface(&source(
            "fn routes() -> Router {\n    let build = || Router::new().route(\"/lazy\", \
             get(lazy));\n    build()\n}"
        ));
        assert_eq!(closured.routes.len(), 1);
        assert!(closured.routes[0].conditional);
    }

    #[test]
    fn a_computed_path_is_counted_as_dynamic_not_guessed() {
        let recovered = surface(&source(
            "fn routes(prefix: &str) -> Router { Router::new().route(prefix, get(handler)) }"
        ));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.dynamic, 1);
    }

    #[test]
    fn an_unresolvable_composition_is_recorded_rather_than_dropped() {
        let recovered = surface(&source(
            "fn app(other: Router) -> Router { Router::new().merge(other) }"
        ));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.unresolved, ["merge"]);
    }

    #[test]
    fn method_router_chains_yield_every_verb() {
        let recovered = surface(&source(
            "fn routes() -> Router { Router::new().route(\"/item\", get(show).post(create)) }"
        ));
        let rows: Vec<(&str, &str)> = recovered
            .routes
            .iter()
            .map(|route| (route.method.as_str(), route.handler.as_str()))
            .collect();
        assert_eq!(rows, [("GET", "show"), ("POST", "create")]);
    }

    #[test]
    fn providers_that_fold_each_other_are_reported_entangled_not_composed() {
        let recovered = surface(&source(
            "fn left() -> Router { Router::new().route(\"/l\", get(l)).merge(right()) }\nfn \
             right() -> Router { Router::new().route(\"/r\", get(r)).merge(left()) }"
        ));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.entangled, ["left", "right"]);
    }

    #[test]
    fn entangled_providers_report_the_registrations_they_discard() {
        let recovered = surface(&source(
            "fn left() -> Router { Router::new().route(\"/l\", get(l)).merge(right()) }\nfn \
             right() -> Router { Router::new().route(\"/r\", get(r)).merge(left()) }"
        ));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.discarded, 2);
    }

    #[test]
    fn same_named_providers_in_two_modules_stay_two_providers() {
        let recovered = surface(&source(
            "mod a { pub fn routes() -> Router { Router::new().route(\"/a\", get(a)) } }\nmod b { \
             pub fn routes() -> Router { Router::new().route(\"/b\", get(b)) } }\nfn app() -> \
             Router { Router::new().nest(\"/x\", a::routes()).nest(\"/y\", b::routes()) }"
        ));
        assert_eq!(recovered.providers, 3);
        assert_eq!(paths(&recovered), ["/x/a", "/y/b"]);
        assert!(recovered.ambiguous.is_empty());
    }

    #[test]
    fn a_composition_naming_no_collected_provider_is_recorded() {
        let recovered = surface(&source(
            "fn app() -> Router { Router::new().route(\"/ok\", get(ok)).merge(missing()) }"
        ));
        assert_eq!(paths(&recovered), ["/ok"]);
        assert_eq!(recovered.unresolved, ["merge"]);
    }

    #[test]
    fn a_router_built_by_a_nested_function_is_not_attributed_to_its_host() {
        let recovered = surface(&source(
            "fn app() -> Router {\n    fn inner() -> Router { Router::new().route(\"/deep\", \
             get(deep)) }\n    Router::new().nest(\"/api\", inner())\n}"
        ));
        assert_eq!(recovered.providers, 2);
        assert_eq!(recovered.composed, 1);
        assert_eq!(paths(&recovered), ["/api/deep"]);
    }

    #[test]
    fn a_subrouter_mounted_under_a_branch_is_conditional() {
        let recovered = surface(&source(
            "fn inner() -> Router { Router::new().route(\"/x\", get(x)) }\nfn app(flag: bool) -> \
             Router {\n    let mut r = Router::new();\n    if flag { r = r.nest(\"/api\", \
             inner()); }\n    r\n}"
        ));
        assert_eq!(paths(&recovered), ["/api/x"]);
        assert!(recovered.routes[0].conditional);
    }

    #[test]
    fn a_nest_with_a_computed_prefix_does_not_leak_its_routes_to_the_root() {
        let recovered = surface(&source(
            "fn inner() -> Router { Router::new().route(\"/list\", get(list)) }\nfn app(prefix: \
             &str) -> Router { Router::new().nest(prefix, inner()) }"
        ));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.providers, 2);
        assert_eq!(recovered.composed, 1);
        assert_eq!(recovered.unplaced, 1);
    }

    #[test]
    fn an_unrecognised_handler_is_not_reported_as_a_computed_path() {
        let recovered = surface(&source(
            "fn routes() -> Router { Router::new().route(\"/x\", handler_service) }"
        ));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.dynamic, 0);
        assert_eq!(recovered.opaque_handler, 1);
    }

    #[test]
    fn framework_vocabulary_on_a_foreign_receiver_is_counted_not_recovered() {
        let recovered = surface(&source(
            "fn inner() -> Router { Router::new().route(\"/x\", get(x)) }\nfn app() -> Router {\n \
             let mut settings = Config::new();\n    settings.nest(\"/cfg\", inner());\n    \
             Router::new().route(\"/ok\", get(ok))\n}"
        ));
        assert_eq!(paths(&recovered), ["/ok", "/x"]);
        assert_eq!(recovered.foreign_receiver, 1);
        assert!(recovered.unresolved.is_empty());
        assert!(recovered.entangled.is_empty());
    }

    #[test]
    fn a_registration_behind_a_cfg_gate_is_conditional() {
        let gated_item = surface(&source(
            "#[cfg(feature = \"x\")]\nfn routes() -> Router { Router::new().route(\"/x\", \
             get(x)) }"
        ));
        assert_eq!(paths(&gated_item), ["/x"]);
        assert!(gated_item.routes[0].conditional);
        assert_eq!(gated_item.conditional, 1);

        let gated_statement = surface(&source(
            "fn routes() -> Router {\n    let mut app = Router::new();\n    #[cfg(feature = \
             \"x\")]\n    {\n        app = app.route(\"/x\", get(x));\n    }\n    app\n}"
        ));
        assert_eq!(paths(&gated_statement), ["/x"]);
        assert!(gated_statement.routes[0].conditional);
    }

    #[test]
    fn a_router_built_in_a_trait_default_method_or_a_const_is_collected() {
        let defaulted = surface(&source(
            "trait Api { fn routes(&self) -> Router { Router::new().route(\"/t\", get(t)) } }"
        ));
        assert_eq!(paths(&defaulted), ["/t"]);
        assert_eq!(defaulted.providers, 1);

        let constant = surface(&source(
            "static APP: Lazy<Router> = Router::new().route(\"/s\", get(s));"
        ));
        assert_eq!(paths(&constant), ["/s"]);
        assert_eq!(constant.providers, 1);
    }

    #[test]
    fn a_source_that_does_not_parse_is_counted_not_silently_skipped() {
        let recovered = surface(&source("fn broken( { "));
        assert!(recovered.routes.is_empty());
        assert_eq!(recovered.unparsed, 1);
    }

    #[test]
    fn conditional_counts_registrations_not_the_endpoints_they_expand_to() {
        let recovered = surface(&source(
            "fn inner(flag: bool) -> Router {\n    let mut r = Router::new();\n    if flag { r = \
             r.route(\"/x\", get(x)); }\n    r\n}\nfn a() -> Router { Router::new().nest(\"/a\", \
             inner(true)) }\nfn b() -> Router { Router::new().nest(\"/b\", inner(false)) }"
        ));
        assert_eq!(paths(&recovered), ["/a/x", "/b/x"]);
        assert_eq!(recovered.conditional, 1);
    }

    #[test]
    fn two_registrations_of_one_method_and_path_are_reported_as_a_conflict() {
        let recovered = surface(&source(
            "fn routes() -> Router { Router::new().route(\"/x\", get(first)).route(\"/x\", \
             get(second)) }"
        ));
        assert_eq!(recovered.routes.len(), 2);
        assert_eq!(recovered.conflicts, ["GET /x"]);
    }

    #[test]
    fn distinct_methods_on_one_path_are_not_a_conflict() {
        let recovered = surface(&source(
            "fn routes() -> Router { Router::new().route(\"/x\", get(show).post(create)) }"
        ));
        assert!(recovered.conflicts.is_empty());
    }
}
