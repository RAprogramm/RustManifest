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
mod library_return;
mod macro_value;
mod method_effect;
mod presence;
mod result;
mod selection;
mod value_path;

pub(super) use self::{
    arithmetic::operation_method,
    flow::{Carried, carried_by},
    library_return::library_return,
    macro_value::{macro_leaves, macro_value},
    method_effect::method_effect,
    presence::{Presence, presence_method},
    result::method_result,
    selection::selects_an_element,
    value_path::value_path
};

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

/// Well-known owning std types by their last path segment: provably not
/// `Copy`, so passing one by value provably moves it.
pub(super) const OWNING_TYPES: [&str; 17] = [
    "Arc",
    "BTreeMap",
    "BTreeSet",
    "Box",
    "File",
    "HashMap",
    "HashSet",
    "Mutex",
    "OsString",
    "PathBuf",
    "Rc",
    "RwLock",
    "String",
    "TcpListener",
    "TcpStream",
    "Vec",
    "VecDeque"
];

/// Returns whether the type with last path segment `name` is a known
/// std owner: provably not `Copy` by std knowledge alone.
pub(super) fn is_owning_type(name: &str) -> bool {
    OWNING_TYPES.contains(&name)
}

/// Well-known external constructor functions by call name, with the
/// concrete type each one produces (`Result`/`Option` wrappers already
/// unwrapped, since these are consumed behind `unwrap`/`?`).
pub(super) const EXTERNAL_CONSTRUCTORS: [(&str, &str); 2] =
    [("tempdir", "TempDir"), ("temp_dir", "PathBuf")];

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{MUTATING_METHODS, consuming_method, is_copy_type};

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
}
