// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Built-in knowledge of the standard library's semantics, encoded as
//! data: which well-known types are `Copy`, which methods mutate their
//! receiver, and which consume it.
//!
//! Checks consult these tables instead of keeping private lists, so the
//! engine's understanding of std lives in one auditable place and grows
//! by adding a line. Classification errs on the safe side for the
//! ownership rules: a method wrongly listed as consuming makes a rule
//! stay silent, never fire falsely.

mod arithmetic;
mod flow;
mod presence;
mod result;
mod selection;

pub(super) use self::{
    arithmetic::operation_method,
    flow::{Carried, carried_by},
    presence::{Presence, presence_method},
    result::method_result,
    selection::selects_an_element
};
use super::behavior::Effect;

/// Well-known always-`Copy` types by their last path segment.
pub(super) const COPY_TYPES: [&str; 26] = [
    "Duration",
    "IpAddr",
    "Ipv4Addr",
    "Ipv6Addr",
    "NonZeroU32",
    "NonZeroU64",
    "NonZeroUsize",
    "Ordering",
    "SocketAddr",
    "bool",
    "char",
    "f32",
    "f64",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "str",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize"
];

/// Methods on std containers, strings, and cells that mutate their
/// receiver in place.
pub(super) const MUTATING_METHODS: [&str; 31] = [
    "append",
    "borrow_mut",
    "clear",
    "dedup",
    "drain",
    "extend",
    "extend_from_slice",
    "fill",
    "get_mut",
    "get_or_insert",
    "insert",
    "insert_str",
    "lock",
    "make_ascii_lowercase",
    "make_ascii_uppercase",
    "pop",
    "push",
    "push_str",
    "remove",
    "replace",
    "reserve",
    "resize",
    "retain",
    "set",
    "sort",
    "sort_by",
    "sort_unstable",
    "store",
    "take",
    "truncate",
    "write"
];

/// Returns whether `method` consumes its receiver: the `into_*` family,
/// leaking, and the `Option`/`Result` adapters that take `self`.
pub(super) fn consuming_method(method: &str) -> bool {
    method.starts_with("into")
        || matches!(
            method,
            "and_then"
                | "expect"
                | "leak"
                | "map"
                | "map_err"
                | "or_else"
                | "unwrap"
                | "unwrap_or"
                | "unwrap_or_default"
                | "unwrap_or_else"
        )
}

/// Returns whether the type with last path segment `name` is provably
/// `Copy` by std knowledge alone.
pub(super) fn is_copy_type(name: &str) -> bool {
    COPY_TYPES.contains(&name)
}

/// Well-known external constructor functions by call name, with the
/// concrete type each one produces (`Result`/`Option` wrappers already
/// unwrapped, since these are consumed behind `unwrap`/`?`).
pub(super) const EXTERNAL_CONSTRUCTORS: [(&str, &str); 2] =
    [("tempdir", "TempDir"), ("temp_dir", "PathBuf")];

/// `(type, method, effect)` rows for std methods that perform the
/// effect when called on an instance — the axiom floor under derived
/// effect chains. Path-pattern sinks catch associated calls like
/// `File::open`; these rows catch the instance calls path patterns
/// cannot see. Only rows the std documentation guarantees; accessors
/// stay out.
pub(super) const METHOD_EFFECTS: [(&str, &str, Effect); 47] = [
    ("Child", "kill", Effect::Process),
    ("Child", "wait", Effect::Process),
    ("Child", "wait_with_output", Effect::Process),
    ("Command", "output", Effect::Process),
    ("Command", "spawn", Effect::Process),
    ("Command", "status", Effect::Process),
    ("DirBuilder", "create", Effect::FileSystem),
    ("File", "flush", Effect::FileSystem),
    ("File", "metadata", Effect::FileSystem),
    ("File", "read", Effect::FileSystem),
    ("File", "read_exact", Effect::FileSystem),
    ("File", "read_to_end", Effect::FileSystem),
    ("File", "read_to_string", Effect::FileSystem),
    ("File", "set_len", Effect::FileSystem),
    ("File", "set_permissions", Effect::FileSystem),
    ("File", "sync_all", Effect::FileSystem),
    ("File", "sync_data", Effect::FileSystem),
    ("File", "try_clone", Effect::FileSystem),
    ("File", "write", Effect::FileSystem),
    ("File", "write_all", Effect::FileSystem),
    ("OpenOptions", "open", Effect::FileSystem),
    ("Stderr", "flush", Effect::ConsoleIo),
    ("Stderr", "write", Effect::ConsoleIo),
    ("Stderr", "write_all", Effect::ConsoleIo),
    ("Stdin", "read_line", Effect::ConsoleIo),
    ("Stdin", "read_to_string", Effect::ConsoleIo),
    ("Stdout", "flush", Effect::ConsoleIo),
    ("Stdout", "write", Effect::ConsoleIo),
    ("Stdout", "write_all", Effect::ConsoleIo),
    ("TcpListener", "accept", Effect::Network),
    ("TcpListener", "incoming", Effect::Network),
    ("TcpStream", "flush", Effect::Network),
    ("TcpStream", "peek", Effect::Network),
    ("TcpStream", "read", Effect::Network),
    ("TcpStream", "read_exact", Effect::Network),
    ("TcpStream", "read_to_end", Effect::Network),
    ("TcpStream", "read_to_string", Effect::Network),
    ("TcpStream", "shutdown", Effect::Network),
    ("TcpStream", "write", Effect::Network),
    ("TcpStream", "write_all", Effect::Network),
    ("UdpSocket", "connect", Effect::Network),
    ("UdpSocket", "peek", Effect::Network),
    ("UdpSocket", "peek_from", Effect::Network),
    ("UdpSocket", "recv", Effect::Network),
    ("UdpSocket", "recv_from", Effect::Network),
    ("UdpSocket", "send", Effect::Network),
    ("UdpSocket", "send_to", Effect::Network)
];

/// Looks up the effect a std method performs on its receiver type.
pub(super) fn method_effect(owner: &str, method: &str) -> Option<Effect> {
    METHOD_EFFECTS
        .iter()
        .find(|(known_owner, known_method, _)| *known_owner == owner && *known_method == method)
        .map(|(_, _, effect)| *effect)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{Effect, MUTATING_METHODS, consuming_method, is_copy_type, method_effect};

    #[test]
    fn scalars_and_known_std_types_are_copy() {
        assert!(is_copy_type("u32"));
        assert!(is_copy_type("Duration"));
        assert!(!is_copy_type("String"));
    }

    #[test]
    fn adapters_and_into_family_consume() {
        assert!(consuming_method("into_bytes"));
        assert!(consuming_method("unwrap"));
        assert!(consuming_method("map_err"));
        assert!(!consuming_method("len"));
    }

    #[test]
    fn mutating_table_is_sorted_for_review() {
        let mut sorted = MUTATING_METHODS;
        sorted.sort_unstable();
        assert_eq!(sorted, MUTATING_METHODS);
    }

    #[test]
    fn std_method_axioms_cover_io_and_spare_accessors() {
        assert_eq!(method_effect("File", "write_all"), Some(Effect::FileSystem));
        assert_eq!(method_effect("TcpStream", "read"), Some(Effect::Network));
        assert_eq!(method_effect("Command", "spawn"), Some(Effect::Process));
        assert_eq!(method_effect("Stdout", "flush"), Some(Effect::ConsoleIo));
        assert_eq!(method_effect("File", "path"), None);
        assert_eq!(method_effect("Vec", "push"), None);
    }

    #[test]
    fn method_effect_table_is_sorted_for_review() {
        let rows: Vec<(&str, &str)> = super::METHOD_EFFECTS
            .iter()
            .map(|(owner, method, _)| (*owner, *method))
            .collect();
        let mut sorted = rows.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, rows);
    }
}
