// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of graph nodes from `syn` items: one module per node kind,
//! one function per item kind. Supporting a new item kind means a new
//! constructor in the module owning that node kind and one match arm at the
//! call sites.

mod impl_node;
mod trait_node;
mod type_node;

pub(super) use self::{
    impl_node::impl_node,
    trait_node::trait_node,
    type_node::{enum_node, node}
};
