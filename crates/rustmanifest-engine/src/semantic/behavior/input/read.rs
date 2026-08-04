// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One observed read of a value from outside the process.

use super::{
    super::{
        output::{Acquisition, Multiplicity, SiteGuard},
        value::Condition
    },
    channel::Channel,
    intake::{Intake, ReadShape, Terminator},
    selector::Selector
};

/// One observed read of a value from outside the process.
///
/// The mirror of a recorded write, field for field: the channel stands
/// where the stream stands, the part drawn where the payload stands, the
/// terminator where the appended one stands, the intake where the conduit
/// stands, and the lock reading is the same reading. The shape and the
/// conditions have no counterpart there, and the mirror is claimed for
/// this record and for no other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRead {
    /// Where the value comes from.
    pub channel:      Channel,
    /// Which part of that channel is drawn.
    pub selector:     Selector,
    /// What shape is taken off it.
    pub shape:        ReadShape,
    /// By what machinery the value reaches the program.
    pub intake:       Intake,
    /// What becomes of the line terminator.
    pub terminator:   Terminator,
    /// Byte range of the reading construct.
    pub site:         rustmanifest_schema::Range,
    /// What stands between the site and unconditional execution.
    pub guard:        SiteGuard,
    /// What each construct standing over the site tests, outermost first.
    pub conditions:   Vec<Condition>,
    /// How many times it runs per call.
    pub multiplicity: Multiplicity,
    /// Where the channel's lock was taken relative to this site, for a
    /// channel that has one.
    pub acquisition:  Acquisition
}
