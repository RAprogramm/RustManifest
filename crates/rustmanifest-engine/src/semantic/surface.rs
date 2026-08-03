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

use std::collections::{BTreeMap, BTreeSet};

use rustmanifest_schema::Range;
use syn::{Attribute, Expr, ImplItem, Item, TraitItem, spanned::Spanned, visit::Visit};

use crate::source::Source;

/// Registration vocabulary of one web framework.
pub struct FrameworkModel {
    /// Framework name, for reporting which model produced a row.
    pub name:          &'static str,
    /// Types whose associated constructor roots a router chain. A
    /// registration method is only framework vocabulary when its receiver
    /// chain reaches one of these, or a binding proven to hold one.
    pub routers:       &'static [&'static str],
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
    routers:       &["Router", "MethodRouter"],
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
    /// Whether the registration, or any composition that mounts it,
    /// stands under a construct that may not run — a branch, a match arm,
    /// a loop, a closure, or a `#[cfg]` gate — so the route exists in some
    /// builds or runs only, and is not presented as unconditional.
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
    pub routes:           Vec<Route>,
    /// Sources that do not parse as Rust, so nothing could be read from
    /// them. A parse failure and a route-free file are different answers.
    pub unparsed:         u32,
    /// Registrations whose path is not a literal — assembled at runtime
    /// and therefore not recoverable by construction.
    pub dynamic:          u32,
    /// Registrations whose path *is* a literal but whose handler argument
    /// is not a recognised method-router, so no verb could be read. The
    /// path is known; the method is not.
    pub opaque_handler:   u32,
    /// Calls using framework vocabulary on a receiver that was never
    /// proven to hold a router value. Counted rather than assumed to be a
    /// registration, and rather than dropped.
    pub foreign_receiver: u32,
    /// Composition arguments that name no resolvable provider function,
    /// with the composing method that held them.
    pub unresolved:       Vec<String>,
    /// Composition arguments matching more than one provider with equal
    /// proximity, so the fold target is undetermined and is not guessed.
    pub ambiguous:        Vec<String>,
    /// Compositions whose mount prefix is not a literal: the sub-router
    /// exists and is not a root, but no path can be composed for it, so
    /// its routes are not emitted at all rather than emitted unprefixed.
    pub unplaced:         usize,
    /// Providers folded into other providers, hence not surface roots
    /// themselves.
    pub composed:         usize,
    /// Functions observed to build a router value.
    pub providers:        usize,
    /// Registrations standing under a construct that may not run. Counted
    /// once per registration call, not once per endpoint it expands to.
    pub conditional:      usize,
    /// `METHOD path` pairs claimed by more than one endpoint. A framework
    /// that rejects duplicate routes cannot serve this surface at all.
    pub conflicts:        Vec<String>,
    /// Providers that fold each other, leaving no root to expand from.
    /// Their composition order is undetermined, so no path is composed
    /// for them — the same refusal the layer inference makes on mutual
    /// module dependencies.
    pub entangled:        Vec<String>,
    /// Registrations proven to exist but emitted nowhere because every
    /// provider holding them is part of a composition cycle.
    pub discarded:        usize
}

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
        let Ok(file) = syn::parse_file(source.text()) else {
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

/// Reports every `METHOD path` claimed by more than one endpoint.
///
/// `routes` must already be sorted by path then method, so competing
/// claims are adjacent.
fn conflicts(routes: &[Route]) -> Vec<String> {
    routes
        .chunk_by(|left, right| left.path == right.path && left.method == right.method)
        .filter(|group| group.len() > 1)
        .filter_map(|group| {
            group
                .first()
                .map(|route| format!("{} {}", route.method, route.path))
        })
        .collect()
}

/// One function that builds a router value.
#[derive(Debug)]
struct Provider {
    /// Full path of the item, module path included.
    path:   Vec<String>,
    /// Path of the item's owner: `path` without its last segment.
    scope:  Vec<String>,
    /// Routes registered directly in this function.
    routes: Vec<Route>,
    /// Compositions this function performs, before target resolution.
    folds:  Vec<RawFold>
}

/// A composition as written, before its argument is resolved.
#[derive(Debug)]
struct RawFold {
    /// Mount prefix; `None` when the prefix argument is not a literal,
    /// `Some("")` for a flat merge.
    prefix:      Option<String>,
    /// Path segments naming the folded provider, as written.
    reference:   Vec<String>,
    /// Whether the composition stands under a construct that may not run.
    conditional: bool,
    /// Composing method, for reporting an unresolvable argument.
    method:      String
}

/// A composition whose target resolved to exactly one provider key.
#[derive(Debug)]
struct Fold {
    /// Mount prefix; `None` when the prefix argument is not a literal.
    prefix:      Option<String>,
    /// Key of the folded provider.
    target:      String,
    /// Whether the composition stands under a construct that may not run.
    conditional: bool
}

/// What a composition argument resolved to.
enum Resolution {
    /// Exactly one provider is the nearest match.
    Unique(String),
    /// No provider carries the referenced path as a suffix.
    Missing,
    /// Several providers match with equal proximity.
    Ambiguous
}

/// Resolves every provider's compositions, recording the ones that name
/// no provider and the ones that name several.
fn resolved_folds(
    providers: &BTreeMap<String, Provider>,
    recovered: &mut Surface
) -> BTreeMap<String, Vec<Fold>> {
    let mut out = BTreeMap::new();
    for (key, provider) in providers {
        let mut folds = Vec::new();
        for raw in &provider.folds {
            match resolve(&raw.reference, &provider.scope, providers) {
                Resolution::Unique(target) => folds.push(Fold {
                    prefix: raw.prefix.clone(),
                    target,
                    conditional: raw.conditional
                }),
                Resolution::Missing => recovered.unresolved.push(raw.method.clone()),
                Resolution::Ambiguous => recovered.ambiguous.push(raw.reference.join("::"))
            }
        }
        out.insert(key.clone(), folds);
    }
    out
}

/// Resolves a written path against the collected providers.
///
/// A provider matches when its full path ends with `reference`, so
/// `api::routes` answers both `routes()` and `api::routes()`. Among the
/// matches, the one sharing the longest module prefix with `scope` wins;
/// a tie is ambiguous and is reported rather than broken arbitrarily.
fn resolve(
    reference: &[String],
    origin: &[String],
    providers: &BTreeMap<String, Provider>
) -> Resolution {
    let mut best: Option<(usize, &String)> = None;
    let mut tied = false;
    for (key, provider) in providers {
        if !provider.path.ends_with(reference) {
            continue;
        }
        let nearness = shared(&provider.scope, origin);
        let Some((top, _)) = best else {
            best = Some((nearness, key));
            tied = false;
            continue;
        };
        if nearness > top {
            best = Some((nearness, key));
            tied = false;
        } else if nearness == top {
            tied = true;
        }
    }
    match best {
        None => Resolution::Missing,
        Some(_) if tied => Resolution::Ambiguous,
        Some((_, key)) => Resolution::Unique(key.clone())
    }
}

/// Length of the common leading run of two module paths.
fn shared(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(one, other)| one == other)
        .count()
}

/// Expansion state: the composed table under construction.
struct Expansion<'a> {
    /// Every collected provider, keyed by full path.
    providers: &'a BTreeMap<String, Provider>,
    /// Resolved compositions per provider key.
    folds:     &'a BTreeMap<String, Vec<Fold>>,
    /// Providers on the current composition chain, to cut cycles.
    seen:      BTreeSet<String>,
    /// Composed endpoints so far.
    routes:    Vec<Route>,
    /// Compositions skipped because their prefix is not a literal.
    unplaced:  usize
}

impl Expansion<'_> {
    /// Expands one provider into fully composed routes.
    fn expand(&mut self, name: &str, prefix: &str, conditional: bool) {
        if !self.seen.insert(name.to_owned()) {
            return;
        }
        let table = self.providers;
        if let Some(provider) = table.get(name) {
            for route in &provider.routes {
                self.routes.push(Route {
                    method:      route.method.clone(),
                    path:        join(prefix, &route.path),
                    handler:     route.handler.clone(),
                    conditional: route.conditional || conditional,
                    source:      route.source,
                    range:       route.range
                });
            }
        }
        let folds = self.folds;
        if let Some(folds) = folds.get(name) {
            for fold in folds {
                if let Some(mount) = fold.prefix.as_ref() {
                    let next = join(prefix, mount);
                    self.expand(&fold.target, &next, conditional || fold.conditional);
                } else {
                    self.unplaced += 1;
                }
            }
        }
        self.seen.remove(name);
    }
}

/// Body of one item that may build a router.
enum Body<'a> {
    /// A function or method body.
    Block(&'a syn::Block),
    /// A `const` or `static` initialiser.
    Value(&'a Expr)
}

/// One item whose body is walked as a candidate provider.
struct Unit<'a> {
    /// Full path of the item, module path included.
    path:    Vec<String>,
    /// The syntax to walk.
    body:    Body<'a>,
    /// Whether the item itself stands behind a `#[cfg]` gate.
    gated:   bool,
    /// Parameter names whose declared type is a router.
    routers: Vec<String>
}

/// Module path a source file implies, from the first path component after
/// the last `src` directory. `lib.rs`, `main.rs` and `mod.rs` name their
/// parent rather than themselves.
fn module_path(path: &std::path::Path) -> Vec<String> {
    let components: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let start = components
        .iter()
        .rposition(|component| component == "src")
        .map_or(0, |at| at + 1);
    let last = components.len().saturating_sub(1);
    let mut out = Vec::new();
    for (offset, component) in components.iter().enumerate().skip(start) {
        if offset < last {
            out.push(component.clone());
            continue;
        }
        let stem = component
            .rsplit_once('.')
            .map_or(component.as_str(), |(stem, _)| stem);
        if !matches!(stem, "lib" | "main" | "mod") {
            out.push(stem.to_owned());
        }
    }
    out
}

/// Appends one segment to a path.
fn appended(scope: &[String], segment: &str) -> Vec<String> {
    let mut out = scope.to_vec();
    out.push(segment.to_owned());
    out
}

/// Collects every item whose body may build a router, including items
/// nested inside inline modules, impls, traits and function bodies. A
/// router built by a nested item is that item's, not its host's.
fn units<'a>(items: &[&'a Item], scope: &[String], gated: bool, out: &mut Vec<Unit<'a>>) {
    for item in items {
        let gated = gated || cfg_gated(item_attributes(item));
        match item {
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    let inner: Vec<&Item> = nested.iter().collect();
                    units(
                        &inner,
                        &appended(scope, &module.ident.to_string()),
                        gated,
                        out
                    );
                }
            }
            Item::Fn(function) => {
                let path = appended(scope, &function.sig.ident.to_string());
                out.push(Unit {
                    path: path.clone(),
                    body: Body::Block(&function.block),
                    gated,
                    routers: router_inputs(&function.sig)
                });
                nested_units(&function.block, &path, gated, out);
            }
            Item::Impl(block) => impl_units(block, scope, gated, out),
            Item::Trait(definition) => trait_units(definition, scope, gated, out),
            Item::Const(constant) => out.push(Unit {
                path: appended(scope, &constant.ident.to_string()),
                body: Body::Value(&constant.expr),
                gated,
                routers: Vec::new()
            }),
            Item::Static(item) => out.push(Unit {
                path: appended(scope, &item.ident.to_string()),
                body: Body::Value(&item.expr),
                gated,
                routers: Vec::new()
            }),
            _ => {}
        }
    }
}

/// Collects the items declared inside a function body under that
/// function's own path.
fn nested_units<'a>(
    block: &'a syn::Block,
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    let mut harvest = ItemHarvest {
        found: Vec::new()
    };
    harvest.visit_block(block);
    if harvest.found.is_empty() {
        return;
    }
    units(&harvest.found, scope, gated, out);
}

/// Collects the members of one inherent or trait impl.
fn impl_units<'a>(
    block: &'a syn::ItemImpl,
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    let owner =
        type_name(&block.self_ty).map_or_else(|| scope.to_vec(), |name| appended(scope, &name));
    for member in &block.items {
        match member {
            ImplItem::Fn(method) => {
                let gated = gated || cfg_gated(&method.attrs);
                let path = appended(&owner, &method.sig.ident.to_string());
                out.push(Unit {
                    path: path.clone(),
                    body: Body::Block(&method.block),
                    gated,
                    routers: router_inputs(&method.sig)
                });
                nested_units(&method.block, &path, gated, out);
            }
            ImplItem::Const(constant) => out.push(Unit {
                path:    appended(&owner, &constant.ident.to_string()),
                body:    Body::Value(&constant.expr),
                gated:   gated || cfg_gated(&constant.attrs),
                routers: Vec::new()
            }),
            _ => {}
        }
    }
}

/// Collects the defaulted members of one trait definition.
fn trait_units<'a>(
    definition: &'a syn::ItemTrait,
    scope: &[String],
    gated: bool,
    out: &mut Vec<Unit<'a>>
) {
    let owner = appended(scope, &definition.ident.to_string());
    for member in &definition.items {
        match member {
            TraitItem::Fn(method) => {
                let Some(block) = method.default.as_ref() else {
                    continue;
                };
                let gated = gated || cfg_gated(&method.attrs);
                let path = appended(&owner, &method.sig.ident.to_string());
                out.push(Unit {
                    path: path.clone(),
                    body: Body::Block(block),
                    gated,
                    routers: router_inputs(&method.sig)
                });
                nested_units(block, &path, gated, out);
            }
            TraitItem::Const(constant) => {
                let Some((_, value)) = constant.default.as_ref() else {
                    continue;
                };
                out.push(Unit {
                    path:    appended(&owner, &constant.ident.to_string()),
                    body:    Body::Value(value),
                    gated:   gated || cfg_gated(&constant.attrs),
                    routers: Vec::new()
                });
            }
            _ => {}
        }
    }
}

/// Collects items declared inside a block without descending into them:
/// each collected item is walked by [`units`] under its own path.
struct ItemHarvest<'ast> {
    /// Items found at any depth of the block.
    found: Vec<&'ast Item>
}

impl<'ast> Visit<'ast> for ItemHarvest<'ast> {
    fn visit_item(&mut self, item: &'ast Item) {
        self.found.push(item);
    }
}

/// Parameter names whose declared type is one of the router types.
fn router_inputs(signature: &syn::Signature) -> Vec<String> {
    let mut out = Vec::new();
    for input in &signature.inputs {
        let syn::FnArg::Typed(typed) = input else {
            continue;
        };
        let (Some(name), Some(kind)) = (binding_name(&typed.pat), type_name(&typed.ty)) else {
            continue;
        };
        if router_type(&kind) {
            out.push(name);
        }
    }
    out
}

/// Last path segment of a type, when the type is a plain path.
fn type_name(kind: &syn::Type) -> Option<String> {
    match kind {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => type_name(&reference.elem),
        syn::Type::Paren(inner) => type_name(&inner.elem),
        syn::Type::Group(inner) => type_name(&inner.elem),
        _ => None
    }
}

/// Whether a type name roots a router chain under any known model.
fn router_type(name: &str) -> bool {
    MODELS.iter().any(|model| model.routers.contains(&name))
}

/// Name bound by a pattern, when the pattern binds exactly one.
fn binding_name(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        _ => None
    }
}

/// Whether a pattern's type annotation names a router type.
fn router_typed(pattern: &syn::Pat) -> bool {
    let syn::Pat::Type(typed) = pattern else {
        return false;
    };
    type_name(&typed.ty).is_some_and(|name| router_type(&name))
}

/// Whether any attribute makes the item's presence configuration-dependent.
fn cfg_gated(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

/// Attributes carried by an item.
fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(node) => &node.attrs,
        Item::Enum(node) => &node.attrs,
        Item::ExternCrate(node) => &node.attrs,
        Item::Fn(node) => &node.attrs,
        Item::ForeignMod(node) => &node.attrs,
        Item::Impl(node) => &node.attrs,
        Item::Macro(node) => &node.attrs,
        Item::Mod(node) => &node.attrs,
        Item::Static(node) => &node.attrs,
        Item::Struct(node) => &node.attrs,
        Item::Trait(node) => &node.attrs,
        Item::TraitAlias(node) => &node.attrs,
        Item::Type(node) => &node.attrs,
        Item::Union(node) => &node.attrs,
        Item::Use(node) => &node.attrs,
        _ => &[]
    }
}

/// Attributes carried by an expression.
///
/// `syn` exposes no accessor for these, and a partial match is how a
/// `#[cfg]` on a statement-position expression goes unnoticed, so every
/// variant that owns attributes is listed.
fn expression_attributes(expression: &Expr) -> &[Attribute] {
    match expression {
        Expr::Array(node) => &node.attrs,
        Expr::Assign(node) => &node.attrs,
        Expr::Async(node) => &node.attrs,
        Expr::Await(node) => &node.attrs,
        Expr::Binary(node) => &node.attrs,
        Expr::Block(node) => &node.attrs,
        Expr::Break(node) => &node.attrs,
        Expr::Call(node) => &node.attrs,
        Expr::Cast(node) => &node.attrs,
        Expr::Closure(node) => &node.attrs,
        Expr::Const(node) => &node.attrs,
        Expr::Continue(node) => &node.attrs,
        Expr::Field(node) => &node.attrs,
        Expr::ForLoop(node) => &node.attrs,
        Expr::Group(node) => &node.attrs,
        Expr::If(node) => &node.attrs,
        Expr::Index(node) => &node.attrs,
        Expr::Infer(node) => &node.attrs,
        Expr::Let(node) => &node.attrs,
        Expr::Lit(node) => &node.attrs,
        Expr::Loop(node) => &node.attrs,
        Expr::Macro(node) => &node.attrs,
        Expr::Match(node) => &node.attrs,
        Expr::MethodCall(node) => &node.attrs,
        Expr::Paren(node) => &node.attrs,
        Expr::Path(node) => &node.attrs,
        Expr::Range(node) => &node.attrs,
        Expr::RawAddr(node) => &node.attrs,
        Expr::Reference(node) => &node.attrs,
        Expr::Repeat(node) => &node.attrs,
        Expr::Return(node) => &node.attrs,
        Expr::Struct(node) => &node.attrs,
        Expr::Try(node) => &node.attrs,
        Expr::TryBlock(node) => &node.attrs,
        Expr::Tuple(node) => &node.attrs,
        Expr::Unary(node) => &node.attrs,
        Expr::Unsafe(node) => &node.attrs,
        Expr::While(node) => &node.attrs,
        Expr::Yield(node) => &node.attrs,
        _ => &[]
    }
}

/// Attributes carried by a statement.
fn statement_attributes(statement: &syn::Stmt) -> &[Attribute] {
    match statement {
        syn::Stmt::Local(local) => &local.attrs,
        syn::Stmt::Item(item) => item_attributes(item),
        syn::Stmt::Expr(expression, _) => expression_attributes(expression),
        syn::Stmt::Macro(macro_statement) => &macro_statement.attrs
    }
}

/// Walks one item body and records what it contributes to the surface,
/// registering it as a provider when it contributes anything.
///
/// Traversal is the syntax visitor's own, so no expression form can be
/// forgotten: a registration inside a branch, a match arm, a closure or
/// a macro-free block is reached exactly like one at the top level. A
/// registration standing under a construct that may not run — including a
/// `#[cfg]` gate on the statement or on the item itself — is recorded as
/// conditional rather than dropped or presented as certain. Items nested
/// in the body are skipped here and walked as providers of their own.
fn collect(
    unit: &Unit<'_>,
    source: usize,
    providers: &mut BTreeMap<String, Provider>,
    recovered: &mut Surface
) {
    let scope = unit
        .path
        .split_last()
        .map_or_else(Vec::new, |(_, rest)| rest.to_vec());
    let mut provider = Provider {
        path:   unit.path.clone(),
        scope:  scope.clone(),
        routes: Vec::new(),
        folds:  Vec::new()
    };
    let mut walk = Walk {
        source,
        conditional: u32::from(unit.gated),
        routers: unit.routers.iter().cloned().collect(),
        provider: &mut provider,
        recovered
    };
    match unit.body {
        Body::Block(block) => walk.visit_block(block),
        Body::Value(expression) => walk.visit_expr(expression)
    }
    if provider.routes.is_empty() && provider.folds.is_empty() {
        return;
    }
    let entry = providers
        .entry(unit.path.join("::"))
        .or_insert_with(|| Provider {
            path: unit.path.clone(),
            scope,
            routes: Vec::new(),
            folds: Vec::new()
        });
    entry.routes.extend(provider.routes);
    entry.folds.extend(provider.folds);
}

/// Visitor collecting registrations and compositions from one body.
struct Walk<'a> {
    /// Index of the source under analysis.
    source:      usize,
    /// Depth of constructs that may not run around the visited node.
    conditional: u32,
    /// Local names proven to hold a router value.
    routers:     BTreeSet<String>,
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

    /// Whether an expression is proven to evaluate to a router value:
    /// a chain rooted in a router type's constructor, or in a binding
    /// already proven to hold one. Anything else is not assumed.
    fn router_expression(&self, expression: &Expr) -> bool {
        match expression {
            Expr::MethodCall(call) => self.router_expression(&call.receiver),
            Expr::Call(call) => router_constructor(&call.func),
            Expr::Path(path) => path
                .path
                .get_ident()
                .is_some_and(|ident| self.routers.contains(&ident.to_string())),
            Expr::Paren(inner) => self.router_expression(&inner.expr),
            Expr::Group(inner) => self.router_expression(&inner.expr),
            Expr::Reference(inner) => self.router_expression(&inner.expr),
            Expr::Try(inner) => self.router_expression(&inner.expr),
            Expr::Await(inner) => self.router_expression(&inner.base),
            _ => false
        }
    }
}

/// Whether a called path is a router type's associated constructor, such
/// as `Router::new` or `axum::Router::<S>::new`.
fn router_constructor(expression: &Expr) -> bool {
    let Expr::Path(path) = expression else {
        return false;
    };
    path.path
        .segments
        .iter()
        .rev()
        .nth(1)
        .is_some_and(|segment| router_type(&segment.ident.to_string()))
}

impl<'ast> Visit<'ast> for Walk<'_> {
    fn visit_item(&mut self, _: &'ast Item) {}

    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if cfg_gated(statement_attributes(statement)) {
            self.under(|walk| syn::visit::visit_stmt(walk, statement));
            return;
        }
        syn::visit::visit_stmt(self, statement);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        syn::visit::visit_local(self, local);
        let Some(name) = binding_name(&local.pat) else {
            return;
        };
        let proven = router_typed(&local.pat)
            || local
                .init
                .as_ref()
                .is_some_and(|init| self.router_expression(&init.expr));
        if proven {
            self.routers.insert(name);
        }
    }

    fn visit_expr_assign(&mut self, assign: &'ast syn::ExprAssign) {
        syn::visit::visit_expr_assign(self, assign);
        if !self.router_expression(&assign.right) {
            return;
        }
        if let Expr::Path(path) = &*assign.left
            && let Some(ident) = path.path.get_ident()
        {
            self.routers.insert(ident.to_string());
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        if let Some(model) = MODELS.iter().copied().find(|model| model.knows(&name)) {
            if self.router_expression(&call.receiver) {
                record(model, &name, call, self);
            } else {
                self.recovered.foreign_receiver += 1;
            }
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
fn record(model: &FrameworkModel, method: &str, call: &syn::ExprMethodCall, walk: &mut Walk<'_>) {
    if model.registrations.contains(&method) {
        record_registration(model, call, walk);
        return;
    }
    record_composition(model, method, call, walk);
}

/// Records one route registration, or the reason it could not be read.
fn record_registration(model: &FrameworkModel, call: &syn::ExprMethodCall, walk: &mut Walk<'_>) {
    let arguments: Vec<&Expr> = call.args.iter().collect();
    let Some(path) = arguments.first().and_then(|argument| literal(argument)) else {
        walk.recovered.dynamic += 1;
        return;
    };
    let handlers = arguments
        .get(1)
        .map(|argument| method_router(model, argument))
        .unwrap_or_default();
    if handlers.is_empty() {
        walk.recovered.opaque_handler += 1;
        return;
    }
    let conditional = walk.conditional > 0;
    if conditional {
        walk.recovered.conditional += 1;
    }
    let span = call.span().byte_range();
    for (verb, handler) in handlers {
        walk.provider.routes.push(Route {
            method: verb,
            path: path.clone(),
            handler,
            conditional,
            source: walk.source,
            range: Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            }
        });
    }
}

/// Records one composition, or the reason its target could not be read.
///
/// A `nest` whose prefix is not a literal still names its sub-router, so
/// the sub-router is recorded as folded — it is not a surface root — and
/// the missing prefix is carried as `None` so expansion counts it instead
/// of emitting the sub-router's routes at the wrong place.
fn record_composition(
    model: &FrameworkModel,
    method: &str,
    call: &syn::ExprMethodCall,
    walk: &mut Walk<'_>
) {
    let arguments: Vec<&Expr> = call.args.iter().collect();
    let nesting = model.nesting.contains(&method);
    let prefix = if nesting {
        arguments.first().and_then(|argument| literal(argument))
    } else {
        Some(String::new())
    };
    let target = if nesting {
        arguments.get(1)
    } else {
        arguments.first()
    };
    let Some(reference) = target.and_then(|argument| provider_reference(argument)) else {
        walk.recovered.unresolved.push(method.to_owned());
        return;
    };
    walk.provider.folds.push(RawFold {
        prefix,
        reference,
        conditional: walk.conditional > 0,
        method: method.to_owned()
    });
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

/// Path segments naming the provider a composition argument calls:
/// `routes()` or `module::routes()`, including a chain built on top of
/// such a call. Leading `crate`/`self`/`super` qualifiers are dropped so
/// the remainder can be matched against provider keys.
fn provider_reference(expression: &Expr) -> Option<Vec<String>> {
    match expression {
        Expr::Call(call) => path_segments(&call.func),
        Expr::MethodCall(call) => provider_reference(&call.receiver),
        Expr::Paren(inner) => provider_reference(&inner.expr),
        Expr::Group(inner) => provider_reference(&inner.expr),
        Expr::Await(inner) => provider_reference(&inner.base),
        Expr::Try(inner) => provider_reference(&inner.expr),
        _ => None
    }
}

/// Segments of a path expression, without its leading path qualifiers.
fn path_segments(expression: &Expr) -> Option<Vec<String>> {
    let Expr::Path(path) = expression else {
        return None;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .skip_while(|segment| matches!(segment.as_str(), "crate" | "self" | "super" | "Self"))
        .collect();
    if segments.is_empty() {
        return None;
    }
    Some(segments)
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
