// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Return-type reading: what one signature's declared return says about
//! the value a call to it yields — the plain type, the success and error
//! types carried inside a `Result`/`Option`, and whether the call
//! returns to its caller at all.

/// Return type of one workspace function as the typing table sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Returned {
    /// Last segment of the written return type, `Self` resolved.
    pub(super) plain:   String,
    /// Success type inside a `Result`/`Option` return, `Self` resolved.
    pub(super) success: Option<String>,
    /// Error type inside a `Result` return, `Self` resolved.
    pub(super) error:   Option<String>,
    /// Element type of a sequence return, where the return declares one.
    pub(super) element: Option<String>,
    /// Whether the signature declares the never type, so a call to this
    /// function does not return to its caller.
    pub(super) never:   bool
}

/// Reads a signature's plain return type and, for `Result`/`Option`
/// returns, the success type carried in the first type argument.
pub(super) fn returned_of(signature: &syn::Signature, owner: Option<&str>) -> Option<Returned> {
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return None;
    };
    if matches!(ty.as_ref(), syn::Type::Never(_)) {
        return Some(Returned {
            plain:   "!".to_owned(),
            success: None,
            error:   None,
            element: None,
            never:   true
        });
    }
    let syn::Type::Path(path) = ty.as_ref() else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let plain = resolve_self(segment.ident.to_string(), owner);
    let success = if segment.ident == "Result" || segment.ident == "Option" {
        type_argument(segment, 0).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    let error = if segment.ident == "Result" {
        type_argument(segment, 1).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    let element = super::signature::element_type(ty)
        .or_else(|| {
            (segment.ident == "Result" || segment.ident == "Option")
                .then(|| argument_type(segment, 0))
                .flatten()
                .and_then(super::signature::element_type)
        })
        .map(|name| resolve_self(name, owner));
    Some(Returned {
        plain,
        success,
        error,
        element,
        never: false
    })
}

/// The `position`-th angle-bracketed type argument itself, where one
/// stands there.
///
/// The last segment answers what a type is called; the type answers what
/// it carries, which is what a sequence inside a fallible return has to
/// be asked for.
fn argument_type(segment: &syn::PathSegment, position: usize) -> Option<&syn::Type> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(written) => Some(written),
            _ => None
        })
        .nth(position)
}

/// Last segment of the `position`-th angle-bracketed type argument.
///
/// Position is counted over the type arguments alone, so a lifetime or a
/// const argument in front of them shifts nothing. A type that carries no
/// name — a unit, a tuple, a reference, a slice — occupies its position
/// and answers `None` for it: the position is taken, and what stands
/// there simply has no last segment to report. Counting only the named
/// ones would slide every later position one place forward and hand back
/// a neighbour's type under this one's name.
fn type_argument(segment: &syn::PathSegment, position: usize) -> Option<String> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let written = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(written) => Some(written),
            _ => None
        })
        .nth(position)?;
    let syn::Type::Path(path) = written else {
        return None;
    };
    path.path.segments.last().map(|s| s.ident.to_string())
}

/// Substitutes `Self` with the impl owner when one is known.
fn resolve_self(name: String, owner: Option<&str>) -> String {
    if name == "Self" {
        owner.map_or(name, str::to_owned)
    } else {
        name
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
    use super::returned_of;

    fn read(signature: &str) -> (String, Option<String>, Option<String>) {
        let parsed: syn::Signature =
            syn::parse_str(signature).expect("the signature parses as written");
        let returned = returned_of(&parsed, None).expect("a written return type is read");
        (returned.plain, returned.success, returned.error)
    }

    /// A success type that carries no name still holds its position. Reading
    /// the failure type in its place is the shape of nearly every binary
    /// entry point, and would put the error type wherever the success type
    /// is expected.
    #[test]
    fn a_nameless_success_type_does_not_yield_its_place_to_the_failure_type() {
        assert_eq!(
            read("fn load() -> Result<(), DbError>"),
            ("Result".to_owned(), None, Some("DbError".to_owned()))
        );
        assert_eq!(
            read("fn load() -> Result<u8, DbError>"),
            (
                "Result".to_owned(),
                Some("u8".to_owned()),
                Some("DbError".to_owned())
            )
        );
    }

    /// A lifetime in front of the type arguments shifts none of them.
    #[test]
    fn a_lifetime_argument_occupies_no_type_position() {
        assert_eq!(
            read("fn borrow<'a>() -> Result<Cow<'a>, DbError>"),
            (
                "Result".to_owned(),
                Some("Cow".to_owned()),
                Some("DbError".to_owned())
            )
        );
    }
}
