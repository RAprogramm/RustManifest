// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What an operation does to its operands.

/// An arithmetic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arithmetic {
    /// Addition.
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
    /// Remainder of a division.
    Remainder,
    /// Negation of one operand.
    Negate
}

impl std::fmt::Display for Arithmetic {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Add => "addition",
            Self::Subtract => "subtraction",
            Self::Multiply => "multiplication",
            Self::Divide => "division",
            Self::Remainder => "remainder",
            Self::Negate => "negation"
        })
    }
}

/// A comparison between two operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Comparison {
    /// Equality.
    Equal,
    /// Inequality.
    NotEqual,
    /// Strictly less.
    Less,
    /// Less or equal.
    LessOrEqual,
    /// Strictly greater.
    Greater,
    /// Greater or equal.
    GreaterOrEqual
}

impl std::fmt::Display for Comparison {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Equal => "equality",
            Self::NotEqual => "inequality",
            Self::Less => "less than",
            Self::LessOrEqual => "less than or equal",
            Self::Greater => "greater than",
            Self::GreaterOrEqual => "greater than or equal"
        })
    }
}

/// A conversion from one representation to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    /// The type the conversion produces, where the source writes it. A
    /// target written nowhere is stated as unidentified, which is a fact
    /// about the reading and never about the call.
    pub target: Option<String>
}

/// What an operation does to its operands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Arithmetic over numbers.
    Arithmetic(Arithmetic),
    /// A comparison answering with a truth value.
    Comparison(Comparison),
    /// A conversion from one representation to another.
    Conversion(Conversion),
    /// One element taken out of a sequence at a written position.
    Selection
}

impl std::fmt::Display for Operation {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arithmetic(kind) => write!(out, "{kind}"),
            Self::Comparison(kind) => write!(out, "{kind}"),
            Self::Conversion(conversion) => match &conversion.target {
                Some(target) => write!(out, "conversion to {target}"),
                None => out.write_str("conversion to a type written nowhere")
            },
            Self::Selection => out.write_str("selection")
        }
    }
}
