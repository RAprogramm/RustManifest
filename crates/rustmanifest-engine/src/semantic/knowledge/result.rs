// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a known call hands back, and whether it can fail.
//!
//! A return type stated as a row is not the same fact as a return type
//! read off a workspace signature: the row states what the library
//! guarantees, and it is consulted only where the workspace itself has no
//! answer. The two are kept apart so a claim can say which one it rests
//! on.
//!
//! Failure is a separate field because it is a separate question. That a
//! call hands back a number says nothing about whether it can decline to,
//! and a conversion that can decline is exactly the shape a program must
//! be written around.

/// Where a call's returned type comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::semantic) enum ReturnForm {
    /// The type written as the call's own type argument, at this
    /// position.
    TypeArgument(u8),
    /// The element type of the receiver.
    Element,
    /// A type the row names outright.
    Named(&'static str)
}

/// Whether a call can fail, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::semantic) enum Outcome {
    /// The call always yields a value.
    Total,
    /// The call yields a failure value rather than a value.
    ///
    /// The failure's own type is the target's, which this row does not
    /// name: naming one would state of every target what holds of the one
    /// the row happened to be written against.
    Fallible,
    /// The call yields no value where there is none to give.
    Optional
}

/// What one known method hands back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::semantic) struct MethodResult {
    /// Type the method is declared on, by its last path segment.
    pub owner:   &'static str,
    /// Method name.
    pub method:  &'static str,
    /// Where its returned type comes from.
    pub form:    ReturnForm,
    /// Whether it can fail.
    pub outcome: Outcome
}

/// Whether a known method converts one representation into another.
///
/// A conversion is a call whose returned type is decided by the caller
/// rather than by the receiver: the caller writes the target and the
/// library produces it. A call answering with the receiver's own type, or
/// with a type the row fixes, transforms or accesses — it does not
/// convert, and reading it as one would put a type in the claim that the
/// source never asked for.
pub(in crate::semantic) const fn converts(result: &MethodResult) -> bool {
    matches!(result.form, ReturnForm::TypeArgument(_))
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

    use crate::{
        semantic::behavior::{Conversion, Guarding, Operation, behavior},
        source::Source
    };

    fn conversion(body: &str) -> Option<(Conversion, Guarding)> {
        let text = format!("fn read(text: &str) -> u32 {{\n{body}\n}}");
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text)];
        behavior(&sources)
            .iter()
            .find(|fact| fact.function == "read")
            .and_then(|fact| {
                fact.operations
                    .iter()
                    .find_map(|site| match &site.operation {
                        Operation::Conversion(conversion) => {
                            Some((conversion.clone(), site.guarding))
                        }
                        _ => None
                    })
            })
    }

    /// A conversion the caller decides the target of is recorded as one,
    /// with the target the source writes and the library's own statement
    /// that it can decline.
    #[test]
    fn a_written_target_is_named_and_the_failure_is_carried() {
        let (conversion, guarding) =
            conversion("    text.parse::<u32>().unwrap_or(0)").expect("a conversion is recorded");
        assert_eq!(conversion.target.as_deref(), Some("u32"));
        assert_eq!(guarding, Guarding::Reported);
    }

    /// A target the source leaves to a later use is stated as
    /// unidentified: reading it off the type a consumer declares would
    /// name a fact about the consumer instead.
    #[test]
    fn a_target_written_nowhere_stays_unidentified() {
        let (conversion, _) =
            conversion("    text.parse().unwrap_or(0)").expect("a conversion is recorded");
        assert!(conversion.target.is_none());
    }

    /// A call answering with a type the row fixes transforms rather than
    /// converts: the caller asked for no type, so naming one would put a
    /// type in the claim the source never wrote.
    #[test]
    fn a_call_the_caller_decides_nothing_about_is_not_a_conversion() {
        assert!(
            conversion("    text.trim().len() as u32")
                .is_some_and(|(conversion, _)| conversion.target.as_deref() == Some("u32"))
        );
    }
}
