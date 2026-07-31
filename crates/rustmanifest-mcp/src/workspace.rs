// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace root confinement for every path accepted over MCP.
//!
//! The server takes its workspace root from the process current directory at
//! startup. Every path parameter received from a client is joined to that
//! root and canonicalized; a canonical result outside the root is rejected,
//! so the server never reads files outside the workspace.

use std::path::{Path, PathBuf};

/// Canonicalized workspace root against which client paths are resolved.
#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    root: PathBuf,
}

impl WorkspaceRoot {
    /// Creates a root from `root`, canonicalizing it once.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] when `root` does not exist or cannot be
    /// canonicalized.
    pub fn new(root: &Path) -> std::io::Result<Self> {
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    /// Creates a root from the process current directory.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] when the current directory is
    /// unavailable or cannot be canonicalized.
    pub fn from_current_dir() -> std::io::Result<Self> {
        Self::new(&std::env::current_dir()?)
    }

    /// Returns the canonical root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Resolves a client-supplied path against the root.
    ///
    /// The path is joined to the root and canonicalized, so symlinks and
    /// `..` components cannot smuggle the result outside the workspace.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError::Escape`] when the canonical result lies
    /// outside the workspace root, and [`ResolveError::Unreadable`] when the
    /// path does not exist or cannot be canonicalized.
    pub fn resolve(&self, requested: &str) -> Result<PathBuf, ResolveError> {
        let joined = self.root.join(requested);
        let canonical = joined.canonicalize().map_err(|source| {
            if escapes_lexically(&self.root, &joined) {
                ResolveError::Escape {
                    requested: requested.to_owned(),
                }
            } else {
                ResolveError::Unreadable {
                    requested: requested.to_owned(),
                    source,
                }
            }
        })?;
        if canonical.starts_with(&self.root) {
            Ok(canonical)
        } else {
            Err(ResolveError::Escape {
                requested: requested.to_owned(),
            })
        }
    }
}

/// Reports whether `joined` leaves `root` after removing `.` and `..`
/// components lexically, without touching the filesystem.
///
/// Used to classify canonicalization failures: a nonexistent path that
/// already points outside the root is an escape attempt, not a read error.
fn escapes_lexically(root: &Path, joined: &Path) -> bool {
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    !normalized.starts_with(root)
}

/// Failure to resolve a client-supplied path inside the workspace root.
#[derive(Debug)]
pub enum ResolveError {
    /// The canonical path lies outside the workspace root.
    Escape {
        /// Path as supplied by the client.
        requested: String,
    },
    /// The path does not exist or cannot be canonicalized.
    Unreadable {
        /// Path as supplied by the client.
        requested: String,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Escape { requested } => {
                write!(formatter, "path `{requested}` escapes workspace root")
            }
            Self::Unreadable { requested, source } => {
                write!(
                    formatter,
                    "path `{requested}` does not exist or is unreadable: {source}"
                )
            }
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Escape { .. } => None,
            Self::Unreadable { source, .. } => Some(source),
        }
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
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn resolves_relative_path_inside_root() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("inside.rs"), "fn main() {}\n").unwrap();
        let root = WorkspaceRoot::new(dir.path()).unwrap();
        let resolved = root.resolve("inside.rs").unwrap();
        assert!(resolved.starts_with(root.path()));
    }

    #[test]
    fn rejects_parent_traversal_to_existing_file() {
        let outer = tempdir().unwrap();
        fs::write(outer.path().join("secret.rs"), "fn main() {}\n").unwrap();
        let inner = outer.path().join("workspace");
        fs::create_dir(&inner).unwrap();
        let root = WorkspaceRoot::new(&inner).unwrap();
        let error = root.resolve("../secret.rs").unwrap_err();
        assert!(matches!(error, ResolveError::Escape { .. }));
        assert!(error.to_string().contains("escapes workspace root"));
    }

    #[test]
    fn rejects_parent_traversal_to_missing_file() {
        let outer = tempdir().unwrap();
        let inner = outer.path().join("workspace");
        fs::create_dir(&inner).unwrap();
        let root = WorkspaceRoot::new(&inner).unwrap();
        let error = root.resolve("../missing.rs").unwrap_err();
        assert!(matches!(error, ResolveError::Escape { .. }));
    }

    #[test]
    fn rejects_absolute_path_outside_root() {
        let outer = tempdir().unwrap();
        fs::write(outer.path().join("secret.rs"), "fn main() {}\n").unwrap();
        let inner = outer.path().join("workspace");
        fs::create_dir(&inner).unwrap();
        let root = WorkspaceRoot::new(&inner).unwrap();
        let outside = outer.path().join("secret.rs");
        let error = root.resolve(&outside.to_string_lossy()).unwrap_err();
        assert!(matches!(error, ResolveError::Escape { .. }));
    }

    #[test]
    fn missing_path_inside_root_is_unreadable() {
        let dir = tempdir().unwrap();
        let root = WorkspaceRoot::new(dir.path()).unwrap();
        let error = root.resolve("missing.rs").unwrap_err();
        assert!(matches!(error, ResolveError::Unreadable { .. }));
    }
}
