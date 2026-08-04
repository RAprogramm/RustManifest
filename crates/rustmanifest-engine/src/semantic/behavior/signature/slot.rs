// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The declared inputs of one signature, one entry each, in order.
//!
//! One entry per input the signature writes, the receiver first, whatever
//! that input is. The profile reading beside this one drops an input it
//! cannot profile — a receiver with no impl type, a pattern that is not a
//! plain name, a type whose last segment does not render — which is right
//! for a profile and wrong for a position: dropping one slides every
//! later input one place forward, and a caller substituting by position
//! would hand argument two to the input written third.

/// The declared inputs of `signature`, one entry each, the receiver
/// first: the name where the input is written as a plain name, and
/// `None` where it is a pattern this tier does not take apart.
pub(in crate::semantic::behavior) fn param_slots(
    signature: &syn::Signature
) -> Vec<Option<String>> {
    signature
        .inputs
        .iter()
        .map(|input| match input {
            syn::FnArg::Receiver(_) => Some("self".to_owned()),
            syn::FnArg::Typed(typed) => match typed.pat.as_ref() {
                syn::Pat::Ident(bound) if bound.subpat.is_none() => Some(bound.ident.to_string()),
                _ => None
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::param_slots;

    fn slots(signature: &str) -> Vec<Option<String>> {
        let parsed: syn::Signature =
            syn::parse_str(signature).expect("the signature parses as written");
        param_slots(&parsed)
    }

    /// Every declared input holds a position, the receiver first, so a
    /// call site's written arguments align with them.
    #[test]
    fn every_declared_input_holds_its_position() {
        assert_eq!(
            slots("fn apply(&self, left: f64, right: f64)"),
            vec![
                Some("self".to_owned()),
                Some("left".to_owned()),
                Some("right".to_owned())
            ]
        );
        assert_eq!(slots("fn free()"), Vec::<Option<String>>::new());
    }

    /// An input this tier cannot name still holds its position: dropping
    /// it would slide every later input one place forward.
    #[test]
    fn an_unnamed_input_keeps_its_place() {
        assert_eq!(
            slots("fn take(_: u8, (left, right): (u8, u8), last: u8)"),
            vec![None, None, Some("last".to_owned())]
        );
    }
}
