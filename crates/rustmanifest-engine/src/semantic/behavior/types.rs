// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Data types of the behavioral fact base: effects, parameter ownership
//! profiles, and per-function fact records. Pure data with display
//! impls — extraction lives beside it, so children of the tier depend on
//! these types without reaching back into the tier root.

use std::fmt;

use rustmanifest_schema::Range;

/// One written path that proved a side effect, with where it stands.
///
/// [`FunctionFacts::effects`] states *what* a function touches; this states
/// *what proved it*, so a claim about a function's capability can cite the
/// call it rests on instead of asserting the category on its own authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSite {
    /// Effect the path proved.
    pub effect: Effect,
    /// The written text that proved the effect: the call path as the
    /// source spells it, or the keyword where a keyword proved it.
    pub proof:  String,
    /// Byte range of that path.
    pub range:  Range
}

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
    ConsoleIo,
    /// Database query provably reading rows: SQL starting with `SELECT`.
    DatabaseRead,
    /// Database statement provably mutating state: SQL starting with a
    /// write keyword (`INSERT`, `UPDATE`, `DELETE`, DDL).
    DatabaseWrite,
    /// Database statement provably sent with its kind unproven: the
    /// query-constructor call is the proof, and the statement's text is
    /// built at runtime — the call is recorded, the kind is not
    /// guessed.
    Database
}

impl fmt::Display for Effect {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unsafe => "unsafe",
            Self::FileSystem => "fs",
            Self::Network => "net",
            Self::Process => "process",
            Self::Environment => "env",
            Self::ConsoleIo => "console",
            Self::DatabaseRead => "db-read",
            Self::DatabaseWrite => "db-write",
            Self::Database => "db"
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
    /// Bit marking a body that can end the current path by panic.
    pub const PANICS: u8 = 1 << 4;

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

    /// Whether the body can end the current path by panic — a
    /// panic-family macro or an explicit process abort observed in it.
    #[must_use]
    pub const fn panics(self) -> bool {
        self.0 & Self::PANICS != 0
    }

    /// Marks the body as one that can panic.
    #[must_use]
    pub const fn with_panic(self) -> Self {
        Self(self.0 | Self::PANICS)
    }
}

/// One observed call with the constructs standing over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    /// Normalized call candidate, in the notation of
    /// [`FunctionFacts::calls`].
    pub candidate:  String,
    /// What stands between this site and unconditional execution.
    pub guard:      super::output::SiteGuard,
    /// Byte range of the calling construct, so a reading written around
    /// it attaches to exactly this site and to no other.
    pub site:       Range,
    /// What the site hands the callee, in written order, the receiver
    /// first where the call has one.
    pub arguments:  super::value::Arguments,
    /// What the source does with a failure the call can hand back.
    pub disposal:   super::value::Disposal,
    /// What each construct standing over the site tests, outermost first,
    /// one entry per guard frame.
    pub conditions: Vec<super::value::Condition>
}

/// One field of a written structure and where its written value came
/// from.
///
/// The construct channel says a type was built; this one says what each
/// field was built *from*, so a shape assembled out of a store can be
/// tied to that store by derivation rather than by co-occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltField {
    /// Type the literal constructs, `Self` resolved.
    pub owner:  String,
    /// Field name as written.
    pub field:  String,
    /// Where the written value came from.
    pub origin: super::value::Provenance
}

/// Behavioral facts of one function or method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacts {
    /// Qualified name: `Type::method` for impl methods, bare name for free
    /// functions.
    pub function:      String,
    /// Index of the source the function was parsed from.
    pub source:        usize,
    /// Byte range of the function's identifier.
    pub range:         Range,
    /// Fields read, as `Owner.field`; `?` owner when the receiver's type
    /// is not resolvable from the signature.
    pub reads:         Vec<String>,
    /// Fields written on pre-existing state, same notation as `reads`;
    /// covers plain and compound assignment plus known mutating methods
    /// on a field expression.
    pub writes:        Vec<String>,
    /// Fields written on values constructed inside the body — assembly
    /// of a fresh value, invisible to callers as mutation.
    pub builds:        Vec<String>,
    /// Types constructed through struct literals.
    pub constructs:    Vec<String>,
    /// Fields of written struct literals with the origin of each written
    /// value, in body order — what a constructed shape was built from.
    pub built_fields:  Vec<BuiltField>,
    /// Error types the signature can return (`Result<_, E>` → `E`).
    pub error_types:   Vec<String>,
    /// Side-effect categories observed, deduplicated and sorted.
    pub effects:       Vec<Effect>,
    /// The paths that proved those categories, one entry per proving site,
    /// sorted by effect then by position.
    pub effect_sites:  Vec<EffectSite>,
    /// Ownership profile of each parameter, in declaration order.
    pub params:        Vec<ParamFacts>,
    /// Declaration flags: trait-impl membership, test marker, external
    /// visibility, extra attributes.
    pub flags:         FunctionFlags,
    /// Call candidates observed in the body, normalized: `name` for a
    /// free-function call, `Type::method` for associated and resolved
    /// method calls, `?::method` when the receiver's type is unknown.
    pub calls:         Vec<String>,
    /// `.clone()` / `.to_owned()` / `.to_string()` call sites observed.
    pub clones:        u32,
    /// Heap allocation sites observed (`Box::new`, `Vec::new`, `vec![]`,
    /// `String::from`, `format!`).
    pub allocations:   u32,
    /// Call sites in body order, each with what stands over it.
    ///
    /// [`Self::calls`] stays the sorted, deduplicated candidate set;
    /// this channel keeps every occurrence, because two calls of the
    /// same function — one under a branch, one not — differ in exactly
    /// the way certainty depends on.
    pub call_sites:    Vec<CallSite>,
    /// One entry per declared input, the receiver first, holding the name
    /// where the input is written as a plain name.
    ///
    /// [`Self::params`] drops an input it cannot profile, so its positions
    /// align with nothing; this list keeps one entry per input so a call
    /// site's written arguments align with it by position.
    pub slots:         Vec<Option<String>>,
    /// Write sites to an output stream, in body order.
    ///
    /// Unlike [`Self::effects`], this channel is never sorted and never
    /// deduplicated: two identical writes are two events, and their
    /// order is part of the fact.
    pub output_writes: Vec<super::output::OutputWrite>,
    /// Sites drawing a value from outside the process, in body order.
    ///
    /// The mirror of [`Self::output_writes`], on the same terms: never
    /// sorted, never deduplicated, one record per site, each carrying
    /// what stands over it. Like the write channel it deliberately does
    /// not agree with [`Self::effects`], which records that the
    /// environment was touched and can record no more.
    pub input_reads:   Vec<super::input::InputRead>,
    /// Sites computing a value from other values, in body order, on the
    /// same terms as every per-site channel here.
    pub operations:    Vec<super::operation::OperationSite>,
    /// Sites choosing among written alternatives, in body order, on the
    /// same terms as every per-site channel here.
    pub choices:       Vec<super::choice::Choice>,
    /// Positions handing a value back to the caller, in body order.
    ///
    /// The tail expression, every written `return`, and every `?`, each
    /// naming where the value it hands back came from. Never sorted,
    /// never deduplicated, one record per position.
    pub handbacks:     Vec<super::handback::Handback>,
    /// Sites producing a process status, in body order.
    ///
    /// A status is observable outside the process exactly as a write is,
    /// and is recorded the same way: one record per site, never
    /// deduplicated, each carrying what stands over it.
    pub exits:         Vec<super::exit::ProcessExit>,
    /// Written database statements, in body order, on the same terms as
    /// every per-site channel here: one record per site, never
    /// deduplicated, the table optional where the text does not prove
    /// one.
    pub queries:       Vec<super::query::QuerySite>
}
