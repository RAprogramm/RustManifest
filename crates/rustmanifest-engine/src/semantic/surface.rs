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
//! framework is a table rather than a code path. Nothing is inferred: a
//! registration whose path is not a literal is counted as dynamic, a
//! composition argument that is not a resolvable provider call is
//! recorded as unresolved, and neither is quietly dropped.

use std::collections::{BTreeMap, BTreeSet};

use rustmanifest_schema::Range;
use syn::{Expr, ImplItem, Item, spanned::Spanned, visit::Visit};

use crate::source::Source;

/// Registration vocabulary of one web framework.
pub struct FrameworkModel {
    /// Framework name, for reporting which model produced a row.
    pub name:          &'static str,
    /// Methods that attach one route to a router value.
    pub registrations: &'static [&'static str],
    /// Calls naming the HTTP method a handler answers.
    pub methods:       &'static [&'static str],
    /// Method that folds another router in under a path prefix.
    pub nesting:       &'static [&'static str],
    /// Method that folds another router in without a prefix.
    pub merging:       &'static [&'static str]
}

/// The axum/tower model: `Router::new().route(path, get(handler))`, with
/// `nest` prefixing a subrouter and `merge` folding one in flat.
pub(super) const AXUM: FrameworkModel = FrameworkModel {
    name:          "axum",
    registrations: &["route"],
    methods:       &[
        "get", "post", "put", "patch", "delete", "head", "options", "trace", "any"
    ],
    nesting:       &["nest"],
    merging:       &["merge"]
};

/// Every framework model the engine knows.
const MODELS: [&FrameworkModel; 1] = [&AXUM];

/// One recovered endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// HTTP method in upper case.
    pub method:      String,
    /// Full path, composed through every enclosing prefix.
    pub path:        String,
    /// Handler function as written at the registration.
    pub handler:     String,
    /// Whether the registration stands under a construct that may not
    /// run — a branch, a match arm, a loop, or a closure — so the route
    /// exists in some builds or runs only, and is not presented as
    /// unconditional.
    pub conditional: bool,
    /// Index of the source the registration lives in.
    pub source:      usize,
    /// Byte range of the registration call.
    pub range:       Range
}

/// The external surface recovered from one source slice.
#[derive(Debug, Default)]
pub struct Surface {
    /// Recovered endpoints, sorted by path then method.
    pub routes:      Vec<Route>,
    /// Registrations whose path is not a literal — assembled at runtime
    /// and therefore not recoverable by construction.
    pub dynamic:     u32,
    /// Composition arguments that name no resolvable provider function,
    /// with the composing method that held them.
    pub unresolved:  Vec<String>,
    /// Providers that fold in others, hence not surface roots themselves.
    pub composed:    usize,
    /// Functions observed to build a router value.
    pub providers:   usize,
    /// Registrations standing under a construct that may not run.
    pub conditional: usize,
    /// Providers that fold each other, leaving no root to expand from.
    /// Their composition order is undetermined, so no path is composed
    /// for them — the same refusal the layer inference makes on mutual
    /// module dependencies.
    pub entangled:   Vec<String>
}

/// Recovers the route table of `sources`.
///
/// Provider functions — those whose body registers or composes routes —
/// are collected first, then expanded from every root: a provider no
/// other provider folds in. Composition cycles are cut on revisit, so a
/// mutually nesting pair yields each route once instead of looping.
#[must_use]
pub fn surface(sources: &[Source]) -> Surface {
    let mut providers: BTreeMap<String, Provider> = BTreeMap::new();
    let mut recovered = Surface::default();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                Item::Fn(function) => {
                    collect(
                        &function.sig.ident.to_string(),
                        &function.block,
                        index,
                        &mut providers,
                        &mut recovered
                    );
                }
                Item::Impl(block) => {
                    for member in &block.items {
                        if let ImplItem::Fn(method) = member {
                            collect(
                                &method.sig.ident.to_string(),
                                &method.block,
                                index,
                                &mut providers,
                                &mut recovered
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let folded: BTreeSet<&str> = providers
        .values()
        .flat_map(|provider| provider.folds.iter())
        .map(|(_, name)| name.as_str())
        .collect();
    recovered.providers = providers.len();
    recovered.composed = folded.len();
    let roots: Vec<&str> = providers
        .keys()
        .map(String::as_str)
        .filter(|name| !folded.contains(name))
        .collect();
    if roots.is_empty() {
        recovered.entangled = providers.keys().cloned().collect();
        return recovered;
    }
    let mut routes = Vec::new();
    for root in roots {
        let mut seen = BTreeSet::new();
        expand(root, "", &providers, &mut seen, &mut routes);
    }
    routes.sort_by(|left, right| {
        (&left.path, &left.method, &left.handler).cmp(&(
            &right.path,
            &right.method,
            &right.handler
        ))
    });
    routes.dedup();
    recovered.conditional = routes.iter().filter(|route| route.conditional).count();
    recovered.routes = routes;
    recovered
}

/// One function that builds a router value.
#[derive(Debug, Default)]
struct Provider {
    /// Routes registered directly in this function.
    routes: Vec<Route>,
    /// `(prefix, provider name)` pairs this function folds in; an empty
    /// prefix marks a flat merge.
    folds:  Vec<(String, String)>
}

/// Walks one function body and records what it contributes to the
/// surface, registering it as a provider when it contributes anything.
///
/// Traversal is the syntax visitor's own, so no expression form can be
/// forgotten: a registration inside a branch, a match arm, a closure or
/// a macro-free block is reached exactly like one at the top level. A
/// registration standing under a construct that may not run is recorded
/// as conditional rather than dropped or presented as certain.
fn collect(
    function: &str,
    block: &syn::Block,
    source: usize,
    providers: &mut BTreeMap<String, Provider>,
    recovered: &mut Surface
) {
    let mut provider = Provider::default();
    let mut walk = Walk {
        source,
        conditional: 0,
        provider: &mut provider,
        recovered
    };
    walk.visit_block(block);
    if provider.routes.is_empty() && provider.folds.is_empty() {
        return;
    }
    let entry = providers.entry(function.to_owned()).or_default();
    entry.routes.extend(provider.routes);
    entry.folds.extend(provider.folds);
}

/// Visitor collecting registrations and compositions from one body.
struct Walk<'a> {
    /// Index of the source under analysis.
    source:      usize,
    /// Depth of constructs that may not run around the visited node.
    conditional: u32,
    /// Provider being filled.
    provider:    &'a mut Provider,
    /// Report counters.
    recovered:   &'a mut Surface
}

impl Walk<'_> {
    /// Visits a node with one more construct that may not run around it.
    fn under<F: FnOnce(&mut Self)>(&mut self, body: F) {
        self.conditional += 1;
        body(self);
        self.conditional -= 1;
    }
}

impl<'ast> Visit<'ast> for Walk<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        if let Some(model) = MODELS.iter().find(|model| model.knows(&name)).copied() {
            record(
                model,
                &name,
                call,
                self.source,
                self.conditional > 0,
                self.provider,
                self.recovered
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.visit_expr(&conditional.cond);
        self.under(|walk| {
            walk.visit_block(&conditional.then_branch);
            if let Some((_, otherwise)) = &conditional.else_branch {
                walk.visit_expr(otherwise);
            }
        });
    }

    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        self.visit_expr(&matched.expr);
        self.under(|walk| {
            for arm in &matched.arms {
                walk.visit_arm(arm);
            }
        });
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.under(|walk| syn::visit::visit_expr_closure(walk, closure));
    }

    fn visit_expr_for_loop(&mut self, loop_expr: &'ast syn::ExprForLoop) {
        self.visit_expr(&loop_expr.expr);
        self.under(|walk| walk.visit_block(&loop_expr.body));
    }

    fn visit_expr_while(&mut self, loop_expr: &'ast syn::ExprWhile) {
        self.visit_expr(&loop_expr.cond);
        self.under(|walk| walk.visit_block(&loop_expr.body));
    }
}

impl FrameworkModel {
    /// Returns whether this model gives the method a meaning.
    fn knows(&self, method: &str) -> bool {
        self.registrations.contains(&method)
            || self.nesting.contains(&method)
            || self.merging.contains(&method)
    }
}

/// Records one registration or composition against the model.
fn record(
    model: &FrameworkModel,
    method: &str,
    call: &syn::ExprMethodCall,
    source: usize,
    conditional: bool,
    provider: &mut Provider,
    recovered: &mut Surface
) {
    let arguments: Vec<&Expr> = call.args.iter().collect();
    if model.registrations.contains(&method) {
        let (Some(path), Some(handlers)) = (
            arguments.first().and_then(|argument| literal(argument)),
            arguments
                .get(1)
                .map(|argument| method_router(model, argument))
        ) else {
            recovered.dynamic += 1;
            return;
        };
        if handlers.is_empty() {
            recovered.dynamic += 1;
            return;
        }
        let span = call.span().byte_range();
        for (verb, handler) in handlers {
            provider.routes.push(Route {
                method: verb,
                path: path.clone(),
                handler,
                conditional,
                source,
                range: Range {
                    start: u32::try_from(span.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(span.end).unwrap_or(u32::MAX)
                }
            });
        }
        return;
    }
    let nesting = model.nesting.contains(&method);
    let prefix = if nesting {
        arguments.first().and_then(|argument| literal(argument))
    } else {
        Some(String::new())
    };
    let folded = if nesting {
        arguments
            .get(1)
            .and_then(|argument| provider_name(argument))
    } else {
        arguments
            .first()
            .and_then(|argument| provider_name(argument))
    };
    match (prefix, folded) {
        (Some(prefix), Some(name)) => provider.folds.push((prefix, name)),
        _ => recovered.unresolved.push(method.to_owned())
    }
}

/// Reads a string literal argument.
fn literal(expression: &Expr) -> Option<String> {
    let Expr::Lit(literal) = expression else {
        return None;
    };
    match &literal.lit {
        syn::Lit::Str(text) => Some(text.value()),
        _ => None
    }
}

/// Reads the `(method, handler)` pairs of a method-router argument,
/// following `get(h).post(other)` chains.
fn method_router(model: &FrameworkModel, expression: &Expr) -> Vec<(String, String)> {
    match expression {
        Expr::Call(call) => {
            let Some(verb) =
                call_name(&call.func).filter(|name| model.methods.contains(&name.as_str()))
            else {
                return Vec::new();
            };
            call.args
                .first()
                .and_then(handler_name)
                .map(|handler| vec![(verb.to_uppercase(), handler)])
                .unwrap_or_default()
        }
        Expr::MethodCall(call) => {
            let verb = call.method.to_string();
            let mut pairs = method_router(model, &call.receiver);
            if model.methods.contains(&verb.as_str())
                && let Some(handler) = call.args.first().and_then(handler_name)
            {
                pairs.push((verb.to_uppercase(), handler));
            }
            pairs
        }
        _ => Vec::new()
    }
}

/// Last path segment of a called function expression.
fn call_name(expression: &Expr) -> Option<String> {
    let Expr::Path(path) = expression else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// Handler name of a method-router argument: a plain path, or the base
/// of a chain such as `handler.layer(..)`.
fn handler_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::MethodCall(call) => handler_name(&call.receiver),
        _ => None
    }
}

/// Provider function named by a composition argument: `routes()` or
/// `module::routes()`, including a chain built on top of such a call.
fn provider_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Call(call) => call_name(&call.func),
        Expr::MethodCall(call) => provider_name(&call.receiver),
        _ => None
    }
}

/// Expands one provider into fully composed routes.
fn expand(
    name: &str,
    prefix: &str,
    providers: &BTreeMap<String, Provider>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<Route>
) {
    if !seen.insert(name.to_owned()) {
        return;
    }
    let Some(provider) = providers.get(name) else {
        return;
    };
    for route in &provider.routes {
        out.push(Route {
            method:      route.method.clone(),
            path:        join(prefix, &route.path),
            handler:     route.handler.clone(),
            conditional: route.conditional,
            source:      route.source,
            range:       route.range
        });
    }
    for (nested_prefix, folded) in &provider.folds {
        expand(folded, &join(prefix, nested_prefix), providers, seen, out);
    }
    seen.remove(name);
}

/// Joins a prefix and a path without doubling or dropping separators.
fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        return path.to_owned();
    }
    if path.is_empty() || path == "/" {
        return prefix.to_owned();
    }
    format!(
        "{}/{}",
        prefix.trim_end_matches('/'),
        path.trim_start_matches('/')
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
    use std::path::PathBuf;

    use super::surface;
    use crate::source::Source;

    fn source(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
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

    #[test]
    fn nesting_composes_prefixes_and_merging_keeps_paths() {
        let recovered = surface(&source(
            "fn inner() -> Router { Router::new().route(\"/list\", get(list)) }\nfn flat() -> \
             Router { Router::new().route(\"/health\", get(health)) }\nfn app() -> Router { \
             Router::new().nest(\"/api/v1\", inner()).merge(flat()) }"
        ));
        let paths: Vec<&str> = recovered
            .routes
            .iter()
            .map(|route| route.path.as_str())
            .collect();
        assert_eq!(paths, ["/api/v1/list", "/health"]);
        assert_eq!(recovered.providers, 3);
        assert_eq!(recovered.composed, 2);
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
        let paths: Vec<&str> = matched
            .routes
            .iter()
            .map(|route| route.path.as_str())
            .collect();
        assert_eq!(paths, ["/full", "/slim"]);
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
}
