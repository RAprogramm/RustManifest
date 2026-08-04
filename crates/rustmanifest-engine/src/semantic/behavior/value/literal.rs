// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! A value written in the source.
//!
//! Kept exactly as written and never evaluated: the digits of a number
//! stay digits, so a claim quotes the source rather than restating it.
//! Evaluating would mean computing, and this tier proves.

/// A value written in the source, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// An integer, with its digits and any suffix as written.
    Integer(String),
    /// A floating-point number, as written.
    Float(String),
    /// A text literal, with escapes already resolved by the parser.
    Text(String),
    /// A byte literal.
    Byte(u8),
    /// A boolean.
    Bool(bool),
    /// A character.
    Char(char),
    /// The unit value, which a returnless tail hands back.
    Unit
}

impl Literal {
    /// The literal as a claim may quote it.
    #[must_use]
    pub fn quoted(&self) -> String {
        match self {
            Self::Integer(digits) | Self::Float(digits) => digits.clone(),
            Self::Text(text) => format!("{text:?}"),
            Self::Byte(byte) => format!("{byte}"),
            Self::Bool(value) => format!("{value}"),
            Self::Char(value) => format!("{value:?}"),
            Self::Unit => "()".to_owned()
        }
    }

    /// The literal a written expression is, where it is one.
    #[must_use]
    pub fn of(expression: &syn::Expr) -> Option<Self> {
        let syn::Expr::Lit(literal) = expression else {
            return None;
        };
        Some(match &literal.lit {
            syn::Lit::Int(number) => Self::Integer(number.to_string()),
            syn::Lit::Float(number) => Self::Float(number.to_string()),
            syn::Lit::Str(text) => Self::Text(text.value()),
            syn::Lit::Byte(byte) => Self::Byte(byte.value()),
            syn::Lit::Bool(value) => Self::Bool(value.value),
            syn::Lit::Char(value) => Self::Char(value.value()),
            _ => return None
        })
    }
}
