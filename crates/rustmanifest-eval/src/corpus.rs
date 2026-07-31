// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Pinned corpus manifest (`eval/corpus.toml`) loading.

use std::path::Path;

use serde::Deserialize;

use crate::error::EvalError;

/// The full corpus manifest: an ordered list of pinned repositories.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Pinned repository entries, sorted by name after loading.
    #[serde(rename = "repo")]
    pub repos: Vec<RepoEntry>
}

/// One pinned repository inside the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoEntry {
    /// Stable corpus entry name; also the report key.
    pub name:  String,
    /// Checkout location relative to the corpus root.
    pub path:  String,
    /// Pinned git commit (full hex) the baseline was blessed against.
    pub rev:   String,
    /// Free-form description of why the repository is in the corpus.
    pub notes: String
}

/// Parses a corpus manifest from TOML text and sorts entries by name so
/// downstream reports are deterministic regardless of manifest order.
///
/// # Errors
///
/// Returns [`EvalError::CorpusParse`] when the text is not a valid corpus
/// manifest; `origin` is only used for error reporting.
pub fn parse(text: &str, origin: &Path) -> Result<Corpus, EvalError> {
    let mut corpus: Corpus = toml::from_str(text).map_err(|source| EvalError::CorpusParse {
        path:   origin.to_path_buf(),
        source: Box::new(source)
    })?;
    corpus
        .repos
        .sort_by(|left, right| left.name.cmp(&right.name));
    Ok(corpus)
}

/// Loads and parses the corpus manifest at `path`.
///
/// # Errors
///
/// Returns [`EvalError::CorpusRead`] when the file cannot be read and
/// [`EvalError::CorpusParse`] when its contents are invalid.
pub fn load(path: &Path) -> Result<Corpus, EvalError> {
    let text = std::fs::read_to_string(path).map_err(|source| EvalError::CorpusRead {
        path: path.to_path_buf(),
        source
    })?;
    parse(&text, path)
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

    use super::*;

    const MANIFEST: &str = r#"
[[repo]]
name = "zeta"
path = "third-party/zeta"
rev = "0123456789abcdef0123456789abcdef01234567"
notes = "second entry"

[[repo]]
name = "alpha"
path = "own/alpha"
rev = "fedcba9876543210fedcba9876543210fedcba98"
notes = "first entry"
"#;

    #[test]
    fn parses_and_sorts_entries_by_name() {
        let corpus = parse(MANIFEST, &PathBuf::from("corpus.toml")).unwrap();
        assert_eq!(corpus.repos.len(), 2);
        assert_eq!(corpus.repos[0].name, "alpha");
        assert_eq!(corpus.repos[0].path, "own/alpha");
        assert_eq!(
            corpus.repos[0].rev,
            "fedcba9876543210fedcba9876543210fedcba98"
        );
        assert_eq!(corpus.repos[0].notes, "first entry");
        assert_eq!(corpus.repos[1].name, "zeta");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let bad = "[[repo]]\nname = \"x\"\npath = \"y\"\nrev = \"z\"\nnotes = \"n\"\nextra = 1\n";
        let err = parse(bad, &PathBuf::from("corpus.toml")).unwrap_err();
        assert!(matches!(err, EvalError::CorpusParse { .. }));
    }

    #[test]
    fn load_reads_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.toml");
        std::fs::write(&path, MANIFEST).unwrap();
        let corpus = load(&path).unwrap();
        assert_eq!(corpus.repos.len(), 2);
    }

    #[test]
    fn missing_manifest_is_a_read_error() {
        let err = load(&PathBuf::from("/nonexistent/corpus.toml")).unwrap_err();
        assert!(matches!(err, EvalError::CorpusRead { .. }));
    }
}
