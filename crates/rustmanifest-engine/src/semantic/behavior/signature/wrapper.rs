// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The inner type a single-generic wrapper declares around one value.

use super::type_name::type_last_segment;

/// Inner type of a single-generic wrapper like `IoSource<TcpStream>`,
/// seen through references: the one angle-bracketed type argument's
/// last segment. Types with zero or several type arguments yield
/// nothing — with more than one candidate the inner type would be a
/// guess.
pub(in crate::semantic::behavior) fn wrapped_type(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Reference(reference) => wrapped_type(&reference.elem),
        syn::Type::Path(path) => {
            let segment = path.path.segments.last()?;
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            let mut inners = arguments.args.iter().filter_map(|argument| {
                if let syn::GenericArgument::Type(inner) = argument {
                    type_last_segment(inner)
                } else {
                    None
                }
            });
            let only = inners.next()?;
            inners.next().is_none().then_some(only)
        }
        _ => None
    }
}
