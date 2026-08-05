// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The library's own spellings of taking one element out of a collection.
//!
//! `map.get(&key)` is not an opaque call: the library guarantees the value
//! it hands back is an element of the receiver, chosen by the key. That is
//! the same fact `sequence[index]` states, written as a method — so it is
//! recorded as the same operation, and the origin of the element reaches
//! back to the collection it came out of instead of stopping at a call the
//! workspace does not define.
//!
//! What a row does **not** state is which element. That depends on what is
//! in the collection at the time, which no reading of the source settles;
//! the operation says the value came out of this collection at this key,
//! and stops there.
//!
//! Rows, not branches. A method absent from the list is not read as a
//! selection, and a namesake of the program's own is not credited with the
//! library's behaviour.

/// Library methods handing back an element of their receiver.
const SELECTION_METHODS: [&str; 5] = ["get", "get_mut", "remove", "first", "last"];

/// Whether a library method hands back an element of its receiver.
pub(in crate::semantic) fn selects_an_element(method: &str) -> bool {
    SELECTION_METHODS.contains(&method)
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

    #[test]
    fn each_written_spelling_takes_an_element_out() {
        assert!(selects_an_element("get"));
        assert!(selects_an_element("get_mut"));
        assert!(selects_an_element("remove"));
        assert!(selects_an_element("first"));
        assert!(selects_an_element("last"));
    }

    #[test]
    fn a_method_no_row_names_takes_nothing_out() {
        assert!(!selects_an_element("len"));
        assert!(!selects_an_element("insert"));
        assert!(!selects_an_element("get_or_insert"));
        assert!(!selects_an_element("clear"));
    }
}
