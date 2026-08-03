// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which recorded functions the output forms prescribe a shape for.
//!
//! A form speaks about code that ships. Test bodies and compile-only
//! fixtures reach no user, so every check filters its facts through this
//! module before it counts anything.

use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Whether the facts of one function describe shipped code, the only
/// code these forms prescribe a shape for.
///
/// Two things disqualify a body. The attribute flag catches an item
/// inside a `#[cfg(test)]` module. The path catches what the attribute
/// cannot: a file compiled into the crate's test target carries no
/// attribute at all — compile-only fixture data for a macro test is a
/// whole file of ordinary items, shipped nowhere and run never — and
/// cargo decides that by directory, so this reads the same directory.
/// The first target directory on the path decides, as everywhere else in
/// this engine; a file under none of them stays judged.
pub(super) fn shipped(sources: &[Source], fact: &FunctionFacts) -> bool {
    if fact.flags.test() {
        return false;
    }
    let Some(source) = sources.get(fact.source) else {
        return false;
    };
    !source
        .path()
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .find_map(|component| match component {
            "tests" => Some(true),
            "src" | "benches" | "examples" => Some(false),
            _ => None
        })
        .unwrap_or(false)
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

    use super::super::{
        literal_format::literal_format_insights, lock_run::lock_run_insights,
        loop_write::loop_write_insights, lost_write_error::lost_write_error_insights
    };
    use crate::source::Source;

    /// Compile-only fixture data for a macro test carries no
    /// `#[cfg(test)]` anywhere — it is a whole file of ordinary items
    /// that only ever reaches a compiler inside a test — so the
    /// attribute flag lets it through and the target directory has to
    /// decide. The identical body under `src` stays judged.
    #[test]
    fn a_file_in_the_test_target_is_not_shipped_code() {
        let body = "fn main() {\n    println!(\"one\");\n    println!(\"two\");\n}";
        let fixture = vec![Source::new(
            PathBuf::from("crates/thing/tests/cases/pass/commands.rs"),
            body.to_owned()
        )];
        assert!(lock_run_insights(&fixture).is_empty());
        assert!(literal_format_insights(&fixture).is_empty());
        assert!(lost_write_error_insights(&fixture).is_empty());

        let looped = vec![Source::new(
            PathBuf::from("crates/thing/tests/cases/pass/loop.rs"),
            "pub fn go(v: &[u8]) {\n    for b in v { println!(\"{b}\"); }\n}".to_owned()
        )];
        assert!(loop_write_insights(&looped).is_empty());

        let shipped = vec![Source::new(
            PathBuf::from("crates/thing/src/main.rs"),
            body.to_owned()
        )];
        assert_eq!(lock_run_insights(&shipped).len(), 1);
        assert_eq!(lost_write_error_insights(&shipped).len(), 1);
    }
}
