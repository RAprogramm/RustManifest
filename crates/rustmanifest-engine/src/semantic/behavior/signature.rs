// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Signature analysis: parameter ownership profiles, declared parameter
//! types, and error types extracted from a function's signature alone.
//! Everything here reads declarations, never bodies.
//!
//! Each reading of a declaration sits in its own submodule beside this
//! file: the parameter list, the return type's error side, the generic
//! bounds, and the type-shape readings — the element a collection
//! carries and the inner type a wrapper holds. Every one of those but
//! the bounds renders a written type as a single name through the
//! shared type vocabulary in `type_name`.

mod bound;
mod element;
mod error_type;
mod parameter;
mod slot;
mod type_name;
mod wrapper;

pub(super) use self::{
    bound::generics_bounds,
    element::{element_type, pair_types},
    error_type::error_types,
    parameter::{param_facts, parameter_elements, parameter_types},
    slot::param_slots,
    type_name::type_last_segment,
    wrapper::wrapped_type
};
