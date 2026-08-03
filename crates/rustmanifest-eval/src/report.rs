// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Report shapes, the comparable section, baseline diffing, and the p95
//! percentile computation.
//!
//! A full [`Report`] contains everything one evaluation run produced.
//! The [`Comparable`] projection is the part diffed against the committed
//! baseline: timings are excluded because wall-clock numbers vary between
//! machines and runs, and the actual checkout revision is excluded because
//! local trees are allowed to drift from the pin.

mod drift;
mod model;
mod percentile;
mod projection;

pub use drift::diff;
pub use model::{Comparable, ComparableRepo, RepoReport, RepoTimings, Report};
pub use percentile::p95;
