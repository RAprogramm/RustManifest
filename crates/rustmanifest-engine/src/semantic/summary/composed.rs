// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where one value came from, with every reference followed.
//!
//! The fact base stores shallow records: a value names another by index
//! into a channel of the same function, so two consumers of one value
//! name one record instead of carrying two copies. A statement cannot be
//! made of indices, so this is the tree the indices unfold into — the
//! same split the write record and the write claim already draw.
//!
//! Unfolding is bounded. A chain of calls has no finite form otherwise,
//! and the bound is a refusal rather than a truncation: past it the term
//! states that the origin continues, which is a different thing from a
//! leaf that reads as an answer.

use crate::semantic::behavior::{Channel, Literal, Operation, OriginGap, ParamSlot, Selector};

/// How deep a composed origin is unfolded.
///
/// This number is this repository's prescription and is admitted as such.
/// What is derived is the refusal at the bottom, never the depth itself.
pub const ORIGIN_DEPTH: u8 = 14;

/// Where one value came from, with every reference followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposedOrigin {
    /// A value written in the source.
    Written(Literal),
    /// The value one of the function's own inputs arrived as, still
    /// unsubstituted.
    Parameter(ParamSlot),
    /// The value a call handed back, with what the site handed it and
    /// where the callee's own answer came from.
    Call {
        /// Normalized candidate the site names.
        candidate: String,
        /// Origins of the arguments the site wrote, in written order.
        arguments: Vec<Self>,
        /// Where the callee's own handback comes from, with this site's
        /// arguments substituted for the callee's inputs.
        carried:   Box<Self>,
        /// Where the callee's failure half comes from, on the same
        /// terms. Read only where the caller takes that half apart, and
        /// never counted when the value read is the success half.
        failed:    Box<Self>,
        /// Type the callee hands back, where a resolver settled it.
        ///
        /// A type is not an origin: knowing that a call answers with a
        /// `bool` says nothing about which `bool`. It is carried anyway
        /// because a reader stuck at a refusal is better served by the
        /// type than by nothing, and because stating it costs no claim.
        returns:   Option<String>
    },
    /// One of several values, whichever the taken position produces.
    Choice(Vec<Self>),
    /// The value a field of a known type holds, named `Owner.field`.
    Field(String),
    /// The value a standard-library path stands for, named as written.
    Constant(String),
    /// A value of a named type built at the position.
    Constructed(String),
    /// The value handed to a closure as one of its own inputs.
    Supplied(String),
    /// The value a call to something outside this program hands back.
    ///
    /// A settled answer and not a refusal: what such a call produces is
    /// decided outside the sources being read, exactly as the content of
    /// a field is decided wherever its type is constructed. Naming the
    /// callee states everything the program itself proves.
    Beyond(String),
    /// A value drawn from outside the process.
    Boundary {
        /// Where it comes from.
        channel:  Channel,
        /// Which part of that channel.
        selector: Selector
    },
    /// A value one operation produced from others.
    Computed {
        /// What the operation does.
        operation: Operation,
        /// Where each operand came from, in written order.
        operands:  Vec<Self>
    },
    /// No value: control leaves the position instead of producing one.
    Leaves,
    /// Not derived, with the reason.
    NotDerived(OriginGap)
}

impl ComposedOrigin {
    /// The alternatives one term amounts to, with a choice of choices
    /// read as the one set it is.
    ///
    /// A call is seen through for the same reason its phrase already is:
    /// the value a call stands for is the value it hands back, and the
    /// step from one to the other says nothing about the program.
    ///
    /// Nesting here is an artefact of how the alternatives were reached —
    /// one function choosing among values another function chose among —
    /// and not a fact about the program: what can stand in that position
    /// is one set, however many steps led to it.
    ///
    /// An alternative that leaves contributes nothing to the set: it
    /// produces no value, so it is not one of the values the position can
    /// hold. That it leaves is recorded on the choice itself, where it is
    /// a fact about control rather than about this value.
    #[must_use]
    pub fn flattened(&self) -> Vec<Self> {
        match self {
            Self::Choice(alternatives) => alternatives
                .iter()
                .filter(|alternative| !matches!(alternative, Self::Leaves))
                .flat_map(Self::flattened)
                .collect(),
            Self::Call {
                carried, ..
            } if !matches!(carried.as_ref(), Self::NotDerived(_)) => carried.flattened(),
            settled => vec![settled.clone()]
        }
    }

    /// Whether the term is settled all the way down.
    #[must_use]
    pub fn derived(&self) -> bool {
        match self {
            Self::Written(_)
            | Self::Parameter(_)
            | Self::Field(_)
            | Self::Constant(_)
            | Self::Constructed(_)
            | Self::Beyond(_)
            | Self::Supplied(_)
            | Self::Leaves
            | Self::Boundary {
                ..
            } => true,
            Self::Computed {
                operands, ..
            } => operands.iter().all(Self::derived),
            Self::Call {
                arguments,
                carried,
                ..
            } => carried.derived() && arguments.iter().all(Self::derived),
            Self::Choice(alternatives) => alternatives.iter().all(Self::derived),
            Self::NotDerived(_) => false
        }
    }

    /// The first question the term failed on, reading outermost first.
    #[must_use]
    pub fn gap(&self) -> Option<OriginGap> {
        match self {
            Self::Written(_)
            | Self::Parameter(_)
            | Self::Field(_)
            | Self::Constant(_)
            | Self::Constructed(_)
            | Self::Beyond(_)
            | Self::Supplied(_)
            | Self::Leaves
            | Self::Boundary {
                ..
            } => None,
            Self::Computed {
                operands, ..
            } => operands.iter().find_map(Self::gap),
            Self::Call {
                arguments,
                carried,
                ..
            } => carried
                .gap()
                .or_else(|| arguments.iter().find_map(Self::gap)),
            Self::Choice(alternatives) => alternatives.iter().find_map(Self::gap),
            Self::NotDerived(gap) => Some(*gap)
        }
    }
}
