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
mod macro_value;
mod method_effect;
mod presence;
mod result;
mod selection;

pub(super) use self::{
    arithmetic::operation_method,
    flow::{Carried, carried_by},
    macro_value::macro_leaves,
    method_effect::method_effect,
    presence::{Presence, presence_method},
    result::method_result,
    selection::selects_an_element
};

/// The language's own primitive types, which are copied rather than
/// moved by the grammar itself.
///
/// This is not library knowledge and it does not grow: the set of
/// primitives is fixed by the language. Whether a library type is copied
/// is the compiler's answer, carried per type by a resolution — two
/// written lists of library names used to answer it here, and both were
/// that answer transcribed by hand, incomplete by construction and
/// silently wrong for every name nobody added.
pub(super) const PRIMITIVE_COPY_TYPES: [&str; 17] = [
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8", "u16",
    "u32", "u64", "u128", "usize"
];

/// Whether `name` is one of the language's own copied primitives.
pub(super) fn is_primitive_copy(name: &str) -> bool {
    PRIMITIVE_COPY_TYPES.contains(&name)
}


#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::is_primitive_copy;

    /// The language's own primitives answer here and nothing else does:
    /// a library type that happens to be copied is the compiler's answer,
    /// and claiming it from a written list is what this replaced.
    #[test]
    fn the_languages_own_primitives_answer_and_no_library_name_is_claimed() {
        assert!(is_primitive_copy("u32"));
        assert!(!is_primitive_copy("Duration"));
        assert!(!is_primitive_copy("String"));
    }

}
