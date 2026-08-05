// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Running the route-table acceptance measurement and writing it out.
//!
//! The command reports and never fails on a low score: the measurement is
//! evidence about the recovery, and turning it into a gate before anyone
//! has read what it says would only invite the numbers to be chosen rather
//! than measured. A repository is reported with no ground truth when it
//! declares no endpoints, which is different from scoring zero.

use std::{io::Write, process::ExitCode};

use crate::{
    cli::RoutesArgs, corpus, error::EvalError, exit::EXIT_CLEAN, routes::RouteAcceptance
};

/// Runs the measurement over the corpus and writes it as JSON.
pub(super) fn run_routes(args: &RoutesArgs) -> Result<ExitCode, EvalError> {
    let corpus = corpus::load(&args.corpus)?;
    let measurement = crate::routes::measure(&corpus, &args.corpus_root)?;
    let text = super::to_pretty_json(&measurement)?;
    std::fs::write(&args.out, text).map_err(|source| EvalError::WriteFile {
        path: args.out.clone(),
        source
    })?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_summary(&mut handle, &measurement)?;
    writeln!(handle, "measurement written to {}", args.out.display())?;
    handle.flush()?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Writes one line per repository: the counts, then the two rates.
fn write_summary(out: &mut impl Write, measurement: &RouteAcceptance) -> Result<(), EvalError> {
    for repo in &measurement.repos {
        let Some(score) = &repo.score else {
            writeln!(
                out,
                "{}: no declared endpoints, nothing to measure",
                repo.name
            )?;
            continue;
        };
        writeln!(
            out,
            "{}: declared {}, recovered {}, agreed {} | precision {} recall {}",
            repo.name,
            score.counts.declared,
            score.counts.recovered,
            score.counts.agreed,
            rate(score.precision()),
            rate(score.recall())
        )?;
    }
    Ok(())
}

/// Renders a rate as a percentage, or as `n/a` where the denominator was
/// zero and no rate exists.
fn rate(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |rate| format!("{:.1}%", rate * 100.0))
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
    use crate::routes::RepoRoutes;

    fn rendered(repos: Vec<RepoRoutes>) -> String {
        let measurement = RouteAcceptance {
            engine_version: "0.0.0".to_owned(),
            repos
        };
        let mut out = Vec::new();
        write_summary(&mut out, &measurement).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_repository_without_ground_truth_says_so() {
        let text = rendered(vec![RepoRoutes {
            name:  "plain".to_owned(),
            score: None
        }]);
        assert!(text.contains("plain: no declared endpoints, nothing to measure"));
    }

    #[test]
    fn a_measured_repository_reports_counts_and_both_rates() {
        let declared = std::iter::once(("GET".to_owned(), "/a".to_owned())).collect();
        let recovered = std::iter::once(("GET".to_owned(), "/a".to_owned())).collect();
        let text = rendered(vec![RepoRoutes {
            name:  "api".to_owned(),
            score: Some(crate::routes::Score::of(&declared, &recovered))
        }]);
        assert!(text.contains("declared 1, recovered 1, agreed 1"));
        assert!(text.contains("precision 100.0% recall 100.0%"));
    }

    #[test]
    fn an_absent_rate_renders_as_not_applicable() {
        assert_eq!(rate(None), "n/a");
        assert_eq!(rate(Some(0.5)), "50.0%");
    }
}
