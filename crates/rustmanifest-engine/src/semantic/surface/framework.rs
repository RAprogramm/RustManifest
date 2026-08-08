// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The registration vocabulary of every web framework the engine knows,
//! held as data so a second framework is a table rather than a code path.

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
    pub merging:       &'static [&'static str],
    /// Types a handler signature takes apart to receive a value the
    /// framework read out of the request path.
    pub path_takers:   &'static [&'static str]
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
    merging:       &["merge"],
    path_takers:   &["Path"]
};

/// Every framework model the engine knows.
pub(super) const MODELS: [&FrameworkModel; 1] = [&AXUM];

impl FrameworkModel {
    /// Returns whether this model gives the method a meaning.
    pub(super) fn knows(&self, method: &str) -> bool {
        self.registrations.contains(&method)
            || self.nesting.contains(&method)
            || self.merging.contains(&method)
    }
}

/// Whether a type name roots a router chain under any known model.
pub(super) fn router_type(name: &str) -> bool {
    MODELS.iter().any(|model| model.routers.contains(&name))
}

/// Whether a handler signature taking this type apart receives a value
/// the framework read out of the request path, under any known model.
#[must_use]
pub(in crate::semantic) fn takes_from_path(name: &str) -> bool {
    MODELS.iter().any(|model| model.path_takers.contains(&name))
}
