// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the library's own types hand back, for the chains written on
//! them: a path asked for its parent hands back a path, a sequence
//! asked to sort itself hands back nothing, and a reading that does not
//! know this leaves every call further along the chain untyped.
//!
//! Only rows the library's own signatures fix are written here, with
//! the fallible and optional wrappers already unwrapped — the reading
//! sees through `?` and `unwrap` elsewhere, and a row that kept the
//! wrapper would answer a question nobody asks. A method whose answer
//! depends on a type parameter the call site chooses is not here: that
//! is what the written turbofish is read for.

/// `(type, method, returned type)` rows the library's own signatures
/// fix, wrappers unwrapped.
const LIBRARY_RETURNS: [(&str, &str, &str); 22] = [
    ("Path", "parent", "Path"),
    ("Path", "join", "PathBuf"),
    ("Path", "with_extension", "PathBuf"),
    ("Path", "to_path_buf", "PathBuf"),
    ("Path", "canonicalize", "PathBuf"),
    ("Path", "file_name", "OsStr"),
    ("Path", "file_stem", "OsStr"),
    ("Path", "extension", "OsStr"),
    ("Path", "to_string_lossy", "String"),
    ("Path", "display", "String"),
    ("PathBuf", "as_path", "Path"),
    ("PathBuf", "parent", "Path"),
    ("PathBuf", "join", "PathBuf"),
    ("PathBuf", "to_string_lossy", "String"),
    ("PathBuf", "display", "String"),
    ("OsStr", "to_string_lossy", "String"),
    ("OsStr", "to_str", "str"),
    ("String", "as_str", "str"),
    ("String", "into_bytes", "Vec"),
    ("str", "to_owned", "String"),
    ("str", "to_string", "String"),
    ("str", "trim", "str")
];

/// The type a library method hands back, where the library fixes it.
pub(in crate::semantic) fn library_return(owner: &str, method: &str) -> Option<&'static str> {
    LIBRARY_RETURNS
        .iter()
        .find(|(known_owner, known_method, _)| *known_owner == owner && *known_method == method)
        .map(|(_, _, returned)| *returned)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::library_return;

    /// A path asked for its parent hands back a path, and the wrapper
    /// the signature writes around it is already unwrapped here.
    #[test]
    fn a_path_hands_back_a_path() {
        assert_eq!(library_return("Path", "parent"), Some("Path"));
        assert_eq!(library_return("Path", "join"), Some("PathBuf"));
        assert_eq!(library_return("Path", "unknown_to_us"), None);
    }
}
