// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Rendering the engine's coverage counters as aligned report lines.

use rustmanifest_engine::Coverage;

use super::layout::line;

/// Renders the aligned coverage report, one output line per element.
pub(super) fn report_lines(report: &Coverage) -> Vec<String> {
    let parsed = format!("{}/{}", report.files_parsed, report.files_total);
    let reachable = format!("{}/{}", report.files_reachable, report.files_total);
    let parse_rate = report.parse_rate();
    let reach_rate = report.reachability_rate();
    let resolution_rate = report.resolution_rate();
    let fields = report.fields_captured;
    let edges = report.aggregation_edges_resolved;
    let resolved = report.use_targets_resolved;
    let external = report.use_targets_external;
    let unresolved = report.use_targets_unresolved;
    let accounted = report.accounted_rate();
    let macros = report.item_macros_seen;
    vec![
        line("files parsed", &parsed, &format!("{parse_rate:.1}%")),
        line(
            "modules mapped",
            &report.modules_total.to_string(),
            &format!(
                "(files reachable from crate roots: {reachable}  {reach_rate:.1}%   standalone \
                 roots: {standalone})",
                standalone = report.files_standalone
            )
        ),
        line(
            "structs captured",
            &report.structs_captured.to_string(),
            &format!(
                "enums {enums}   traits {traits}   fields {fields}",
                enums = report.enums_captured,
                traits = report.traits_captured
            )
        ),
        line(
            "impls captured",
            &report.impls_captured.to_string(),
            &format!(
                "methods {methods}   aggregation edges {edges}   interaction edges {interactions}",
                methods = report.methods_captured,
                interactions = report.interaction_edges_resolved
            )
        ),
        line(
            "use targets",
            &report.use_targets_total.to_string(),
            &format!(
                "resolved in-workspace {resolved}  {resolution_rate:.1}%   external \
                 {external}   unresolved {unresolved}   accounted {accounted:.1}%"
            )
        ),
        line(
            "call candidates",
            &report.call_candidates_total.to_string(),
            &format!(
                "resolved in-workspace {calls_resolved}  {call_rate:.1}%   external \
                 {calls_external}   unknown receiver {calls_unknown}   named elsewhere \
                 {calls_unreachable}   ambiguous {calls_ambiguous}   owner without member \
                 {calls_owned}   accounted {call_accounted:.1}%{scip_tail}",
                calls_resolved = report.call_candidates_resolved,
                call_rate = report.call_resolution_rate(),
                calls_external = report.call_candidates_external,
                calls_unknown = report.call_candidates_unknown,
                calls_unreachable = report.call_candidates_unreachable,
                calls_ambiguous = report.call_candidates_ambiguous,
                calls_owned = report.call_candidates_owned,
                call_accounted = report.call_accounted_rate(),
                scip_tail = if report.call_candidates_scip == 0 {
                    String::new()
                } else {
                    format!("   scip edges +{}", report.call_candidates_scip)
                }
            )
        ),
        line(
            "value origins",
            &report.origins_total.to_string(),
            &format!(
                "derived {derived}  {origin_rate:.1}%   refused {refused}   (a refusal is \
                 recorded and named, so a count of records is not a count of answers)",
                derived = report.origins_derived,
                origin_rate = report.origin_rate(),
                refused = report.origins_total - report.origins_derived
            )
        ),
        line(
            "derives seen",
            &report.derives_seen.to_string(),
            &format!("item macros seen {macros}   (expansions not analyzed)")
        ),
    ]
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use rustmanifest_engine::Coverage;

    use super::report_lines;

    #[test]
    fn report_lines_are_aligned_and_honest() {
        let report = Coverage {
            files_total:                 318,
            files_parsed:                318,
            modules_total:               212,
            files_reachable:             296,
            files_standalone:            22,
            structs_captured:            509,
            enums_captured:              41,
            traits_captured:             12,
            impls_captured:              480,
            methods_captured:            1920,
            fields_captured:             2140,
            aggregation_edges_resolved:  87,
            interaction_edges_resolved:  133,
            use_targets_total:           4044,
            use_targets_resolved:        1620,
            use_targets_external:        2400,
            use_targets_unresolved:      24,
            derives_seen:                233,
            item_macros_seen:            12,
            call_candidates_total:       5000,
            call_candidates_resolved:    1066,
            call_candidates_external:    3600,
            call_candidates_unknown:     200,
            call_candidates_unreachable: 90,
            call_candidates_ambiguous:   30,
            call_candidates_owned:       14,
            call_candidates_scip:        0,
            origins_total:               1400,
            origins_derived:             1050
        };
        let lines = report_lines(&report);
        assert_eq!(lines.len(), 8);
        assert_eq!(lines[0], "files parsed        318/318   100.0%");
        assert_eq!(
            lines[1],
            "modules mapped      212       (files reachable from crate roots: 296/318  93.1%   \
             standalone roots: 22)"
        );
        assert_eq!(
            lines[2],
            "structs captured    509       enums 41   traits 12   fields 2140"
        );
        assert_eq!(
            lines[3],
            "impls captured      480       methods 1920   aggregation edges 87   interaction \
             edges 133"
        );
        assert_eq!(
            lines[4],
            "use targets         4044      resolved in-workspace 1620  40.1%   external 2400   \
             unresolved 24   accounted 99.4%"
        );
        assert_eq!(
            lines[5],
            "call candidates     5000      resolved in-workspace 1066  21.3%   external 3600   \
             unknown receiver 200   named elsewhere 90   ambiguous 30   owner without member \
             14   accounted 95.1%"
        );
        assert_eq!(
            lines[6],
            "value origins       1400      derived 1050  75.0%   refused 350   (a refusal is \
             recorded and named, so a count of records is not a count of answers)"
        );
        assert_eq!(
            lines[7],
            "derives seen        233       item macros seen 12   (expansions not analyzed)"
        );
    }

    #[test]
    fn empty_report_renders_zero_rates() {
        let lines = report_lines(&Coverage::default());
        assert_eq!(lines[0], "files parsed        0/0       0.0%");
    }

    #[test]
    fn scip_edges_appear_only_when_an_index_added_some() {
        let report = Coverage {
            call_candidates_total: 10,
            call_candidates_resolved: 4,
            call_candidates_scip: 3,
            ..Coverage::default()
        };
        let lines = report_lines(&report);
        assert!(lines[5].ends_with("accounted 40.0%   scip edges +3"));
    }
}
