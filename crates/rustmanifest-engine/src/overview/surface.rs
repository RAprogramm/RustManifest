// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The external contract recovered from framework registration calls, plus
//! the honesty counters naming what stayed out of reach: runtime-assembled
//! paths, unresolvable compositions and mutually folding routers.

use super::fragment::push;
use crate::source::Source;

/// Prints the external contract recovered from framework registration
/// calls: method, path, and handler per endpoint, with the honesty
/// counters that state what could not be recovered.
pub(super) fn surface_section(out: &mut String, sources: &[Source], top: usize) {
    let recovered = crate::semantic::surface(sources);
    if recovered.routes.is_empty()
        && recovered.dynamic == 0
        && recovered.unresolved.is_empty()
        && recovered.entangled.is_empty()
    {
        return;
    }
    out.push_str("## External surface\n\n");
    push(
        out,
        &format!(
            "- {} endpoint(s) from {} router-building function(s)\n",
            recovered.routes.len(),
            recovered.providers
        )
    );
    if recovered.conditional > 0 {
        push(
            out,
            &format!(
                "- {} registration(s) standing under a branch, arm, loop or closure — present \
                 only where that construct runs\n",
                recovered.conditional
            )
        );
    }
    if recovered.dynamic > 0 {
        push(
            out,
            &format!(
                "- {} registration(s) with a path assembled at runtime — not \
                 recoverable\n",
                recovered.dynamic
            )
        );
    }
    if !recovered.unresolved.is_empty() {
        push(
            out,
            &format!(
                "- {} composition(s) naming no resolvable router function\n",
                recovered.unresolved.len()
            )
        );
    }
    if !recovered.entangled.is_empty() {
        push(
            out,
            &format!(
                "- router functions folding each other, composition order undetermined: \
                 {}\n",
                recovered.entangled.join(", ")
            )
        );
    }
    for route in recovered.routes.iter().take(top) {
        let file = sources
            .get(route.source)
            .map_or_else(String::new, |source| format!(" ({})", source.short_path()));
        push(
            out,
            &format!(
                "- `{} {}` → `{}`{}{file}\n",
                route.method,
                route.path,
                route.handler,
                if route.conditional {
                    " (conditional)"
                } else {
                    ""
                }
            )
        );
    }
    if recovered.routes.len() > top {
        push(
            out,
            &format!("- ... {} more endpoint(s)\n", recovered.routes.len() - top)
        );
    }
    out.push('\n');
}
