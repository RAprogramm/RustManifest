// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Data types of the behavioral fact base: effects, parameter ownership
//! profiles, and per-function fact records. Pure data with display
//! impls — extraction lives beside it, so children of the tier depend on
//! these types without reaching back into the tier root.

use std::fmt;

use rustmanifest_schema::Range;

/// Side-effect category observed in a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Effect {
    /// `unsafe` block or `unsafe fn` signature.
    Unsafe,
    /// Filesystem access through `std::fs` or `File` operations.
    FileSystem,
    /// Network access through `std::net` socket types.
    Network,
    /// Child-process control through `std::process`.
    Process,
    /// Environment access through `std::env`.
    Environment,
    /// Console output through print-family macros.
    ConsoleIo
}

impl fmt::Display for Effect {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unsafe => "unsafe",
            Self::FileSystem => "fs",
            Self::Network => "net",
            Self::Process => "process",
            Self::Environment => "env",
            Self::ConsoleIo => "console"
        };
        out.write_str(name)
    }
}

/// How one parameter takes and uses its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamFacts {
    /// Parameter name as written.
    pub name:        String,
    /// Last segment of the declared type.
    pub type_name:   String,
    /// How the parameter receives the value.
    pub mode:        ParamMode,
    /// Total observed uses of the parameter in the body.
    pub uses:        u32,
    /// Uses that are only field reads on the parameter.
    pub field_reads: u32
}

/// Ownership mode of a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// Taken by value: the caller gives up ownership.
    Owned,
    /// Taken by shared reference.
    Borrowed,
    /// Taken by mutable reference.
    BorrowedMut
}

impl fmt::Display for ParamMode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Owned => "owned",
            Self::Borrowed => "borrowed",
            Self::BorrowedMut => "borrowed-mut"
        })
    }
}

/// Behavioral facts of one function or method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    /// Qualified name: `Type::method` for impl methods, bare name for free
    /// functions.
    pub function:    String,
    /// Index of the source the function was parsed from.
    pub source:      usize,
    /// Byte range of the function's identifier.
    pub range:       Range,
    /// Fields read, as `Owner.field`; `?` owner when the receiver's type
    /// is not resolvable from the signature.
    pub reads:       Vec<String>,
    /// Fields written, same notation as `reads`; covers plain and compound
    /// assignment plus known mutating methods on a field expression.
    pub writes:      Vec<String>,
    /// Types constructed through struct literals.
    pub constructs:  Vec<String>,
    /// Error types the signature can return (`Result<_, E>` → `E`).
    pub error_types: Vec<String>,
    /// Side-effect categories observed, deduplicated and sorted.
    pub effects:     Vec<Effect>,
    /// Ownership profile of each parameter, in declaration order.
    pub params:      Vec<ParamFacts>,
    /// Whether the function is a method of a trait impl, where the
    /// signature is dictated by the trait.
    pub trait_impl:  bool,
    /// Whether the function carries a `#[test]`-family attribute.
    pub test:        bool,
    /// Whether the function is declared `pub` (any `pub(...)` form).
    pub public:      bool,
    /// Call candidates observed in the body, normalized: `name` for a
    /// free-function call, `Type::method` for associated and resolved
    /// method calls, `?::method` when the receiver's type is unknown.
    pub calls:       Vec<String>,
    /// `.clone()` / `.to_owned()` / `.to_string()` call sites observed.
    pub clones:      u32,
    /// Heap allocation sites observed (`Box::new`, `Vec::new`, `vec![]`,
    /// `String::from`, `format!`).
    pub allocations: u32
}
