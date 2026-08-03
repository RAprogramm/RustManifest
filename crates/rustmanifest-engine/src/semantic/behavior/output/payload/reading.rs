// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recovering a payload from the syntax of one writing construct: a
//! print-family invocation, a format-argument list whose destination is
//! already removed, or a byte slice handed to a writer.
//!
//! Nothing here evaluates, folds, or propagates an argument: a payload
//! either quotes what the source proves or names the reason it cannot.

use super::{OpaqueReason, Payload, segmentation::template};

/// Reads what a print-family macro invocation writes.
///
/// The first argument must be a string literal for any text to be
/// proven: a literal without placeholders is the payload verbatim, a
/// literal with placeholders becomes proven text interleaved with holes,
/// and anything else is opaque with the reason recorded. Argument
/// expressions are kept as provenance only — none of them is evaluated,
/// folded, or propagated, so a payload never claims a value the analysis
/// cannot see in the source.
///
/// A debugging macro is always opaque: its rendering embeds the source
/// position, which this tier does not compute.
pub(in crate::semantic::behavior) fn macro_payload(
    invocation: &syn::Macro,
    debug_macro: bool
) -> Payload {
    if debug_macro {
        return Payload::Opaque(OpaqueReason::DebugMacro);
    }
    let Ok(arguments) = invocation.parse_body_with(
        syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
    ) else {
        return Payload::Opaque(OpaqueReason::Unparsed);
    };
    let written: Vec<&syn::Expr> = arguments.iter().collect();
    argument_payload(&written)
}

/// Reads what a format-argument list writes, the destination of a
/// writing construct already removed.
///
/// The same rule the print family follows, applied one argument later:
/// the first remaining argument must be a string literal for any text to
/// be proven, and the rest are provenance for the holes and nothing
/// more.
pub(in crate::semantic::behavior) fn argument_payload(arguments: &[&syn::Expr]) -> Payload {
    let Some((first, rest)) = arguments.split_first() else {
        return Payload::Empty;
    };
    let syn::Expr::Lit(literal) = first else {
        return Payload::Opaque(OpaqueReason::NonLiteralFormat);
    };
    let text = match &literal.lit {
        syn::Lit::Str(value) => value.value(),
        syn::Lit::ByteStr(value) => String::from_utf8_lossy(&value.value()).into_owned(),
        _ => return Payload::Opaque(OpaqueReason::NonLiteralFormat)
    };
    let rest: Vec<String> = rest
        .iter()
        .map(|argument| quote::quote!(#argument).to_string())
        .collect();
    template(&text, &rest)
}

/// Reads what a byte-slice write puts on its stream.
///
/// There is no format string here: the bytes are the payload. Written as
/// a literal they are proven verbatim; written as anything else they are
/// a value this tier does not evaluate, which is the one reason a
/// payload can be unreadable without a format string being involved.
pub(in crate::semantic::behavior) fn literal_bytes_payload(argument: &syn::Expr) -> Payload {
    let syn::Expr::Lit(literal) = argument else {
        return Payload::Opaque(OpaqueReason::RuntimeValue);
    };
    match &literal.lit {
        syn::Lit::Str(value) => Payload::Literal(value.value()),
        syn::Lit::ByteStr(value) => {
            Payload::Literal(String::from_utf8_lossy(&value.value()).into_owned())
        }
        _ => Payload::Opaque(OpaqueReason::RuntimeValue)
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
    use super::{
        super::{HoleSource, Segment},
        OpaqueReason, Payload
    };

    fn payload_of(source: &str) -> Payload {
        let invocation: syn::Macro = syn::parse_str(source).unwrap();
        let debug = invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "dbg");
        super::macro_payload(&invocation, debug)
    }

    #[test]
    fn a_literal_without_placeholders_is_proven_verbatim() {
        assert_eq!(
            payload_of(r#"println!("Hello, world!")"#),
            Payload::Literal("Hello, world!".to_owned())
        );
        assert_eq!(
            payload_of(r#"print!("{{literal}}")"#),
            Payload::Literal("{literal}".to_owned())
        );
    }

    #[test]
    fn placeholders_become_holes_that_keep_their_argument_as_provenance() {
        let Payload::Template(segments) = payload_of(r#"println!("count: {}", total)"#) else {
            panic!("a placeholder must yield a template");
        };
        assert_eq!(segments[0], Segment::Text("count: ".to_owned()));
        let Segment::Hole(hole) = &segments[1] else {
            panic!("second segment must be a hole");
        };
        assert_eq!(hole.source, HoleSource::Positional(0));
        assert_eq!(hole.expr.as_deref(), Some("total"));

        let Payload::Template(named) = payload_of(r#"println!("hi {name}")"#) else {
            panic!("a captured identifier must yield a template");
        };
        let Segment::Hole(hole) = &named[1] else {
            panic!("second segment must be a hole");
        };
        assert_eq!(hole.source, HoleSource::Named("name".to_owned()));

        let Payload::Template(spec) = payload_of(concat!("println!(\"{0:>", "8.3}\", value)"))
        else {
            panic!("an indexed placeholder must yield a template");
        };
        let Segment::Hole(hole) = &spec[0] else {
            panic!("first segment must be a hole");
        };
        assert_eq!(hole.source, HoleSource::Indexed(0));
        assert_eq!(hole.spec, ">8.3");
    }

    #[test]
    fn anything_unproven_says_why_instead_of_guessing() {
        assert_eq!(
            payload_of("println!(GREETING)"),
            Payload::Opaque(OpaqueReason::NonLiteralFormat)
        );
        assert_eq!(
            payload_of("dbg!(value)"),
            Payload::Opaque(OpaqueReason::DebugMacro)
        );
        assert_eq!(
            payload_of(r#"println!("{")"#),
            Payload::Opaque(OpaqueReason::Unparsed)
        );
        assert_eq!(payload_of("println!()"), Payload::Empty);
    }
}
