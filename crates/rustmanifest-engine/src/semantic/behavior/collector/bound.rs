// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the names a pattern binds stand for, as a resolver settled it.
//!
//! A loop variable, a closure input and an arm's binding are all written
//! without a type, and what each one holds is decided by the function
//! handing it over — an iterator's `next`, an adapter's contract, a
//! variant's payload. The compiler states all of them at the binding
//! itself, in one answer that needs no vocabulary of adapter names and
//! holds for a library nobody wrote a row for.
//!
//! Where a resolver settled nothing, [`element`] reads the written types
//! instead. That reading follows the spellings a source happened to use,
//! which is why it is the fallback and not the answer.
//!
//! [`element`]: super::element

use super::FactCollector;

impl FactCollector {
    /// Types every name `pattern` binds from what a resolver settled at
    /// the binding, answering whether it settled any of them.
    ///
    /// The answer is what lets the written-text reading stand down: a
    /// pattern the compiler typed needs no second opinion, and one it did
    /// not is left to whatever the source spells.
    pub(super) fn bind_pattern(&mut self, pattern: &syn::Pat) -> bool {
        let mut bound = Vec::new();
        idents_of(pattern, &mut bound);
        let mut settled = false;
        for ident in bound {
            let span = ident.span().byte_range();
            let (Ok(start), Ok(end)) = (u32::try_from(span.start), u32::try_from(span.end)) else {
                continue;
            };
            if let Some(declared) = self.type_paths.get(&(start, end)).cloned() {
                self.param_paths.insert(ident.to_string(), declared);
            }
            if let Some(named) = self.typed.get(&(start, end)).cloned() {
                self.param_types.insert(ident.to_string(), named);
                settled = true;
            }
        }
        settled
    }
}

/// Every name a pattern binds, however deeply it destructures.
fn idents_of(pattern: &syn::Pat, into: &mut Vec<syn::Ident>) {
    match pattern {
        syn::Pat::Ident(bound) => {
            into.push(bound.ident.clone());
            if let Some((_, inner)) = &bound.subpat {
                idents_of(inner, into);
            }
        }
        syn::Pat::Tuple(tuple) => {
            for held in &tuple.elems {
                idents_of(held, into);
            }
        }
        syn::Pat::TupleStruct(tuple) => {
            for held in &tuple.elems {
                idents_of(held, into);
            }
        }
        syn::Pat::Struct(fields) => {
            for field in &fields.fields {
                idents_of(&field.pat, into);
            }
        }
        syn::Pat::Slice(slice) => {
            for held in &slice.elems {
                idents_of(held, into);
            }
        }
        syn::Pat::Reference(borrowed) => idents_of(&borrowed.pat, into),
        syn::Pat::Type(typed) => idents_of(&typed.pat, into),
        syn::Pat::Paren(inner) => idents_of(&inner.pat, into),
        syn::Pat::Or(alternatives) => {
            for held in &alternatives.cases {
                idents_of(held, into);
            }
        }
        _ => {}
    }
}
