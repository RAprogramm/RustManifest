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

/// Declaration flags of one function, packed to stay a single value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FunctionFlags(u8);

impl FunctionFlags {
    /// Bit marking a trait-impl method.
    pub const TRAIT_IMPL: u8 = 1;
    /// Bit marking a `#[test]`-family function.
    pub const TEST: u8 = 1 << 1;
    /// Bit marking external `pub` visibility.
    pub const PUBLIC: u8 = 1 << 2;
    /// Bit marking a non-doc, non-test attribute.
    pub const ATTRIBUTED: u8 = 1 << 3;

    /// Builds the flag set from `Self::*` bit constants or-ed together.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Whether the function is a method of a trait impl, where the
    /// signature is dictated by the trait.
    #[must_use]
    pub const fn trait_impl(self) -> bool {
        self.0 & Self::TRAIT_IMPL != 0
    }

    /// Whether the function carries a `#[test]`-family attribute.
    #[must_use]
    pub const fn test(self) -> bool {
        self.0 & Self::TEST != 0
    }

    /// Whether the function is visible outside the crate (plain `pub`);
    /// restricted forms like `pub(crate)` stay `false`.
    #[must_use]
    pub const fn public(self) -> bool {
        self.0 & Self::PUBLIC != 0
    }

    /// Whether the function carries any attribute besides docs and the
    /// `test` family — an attribute can mean an external entry point.
    #[must_use]
    pub const fn attributed(self) -> bool {
        self.0 & Self::ATTRIBUTED != 0
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
    /// Declaration flags: trait-impl membership, test marker, external
    /// visibility, extra attributes.
    pub flags:       FunctionFlags,
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
