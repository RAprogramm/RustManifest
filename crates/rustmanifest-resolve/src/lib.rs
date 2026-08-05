// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Name and type resolution for the engine, read from a loaded cargo
//! workspace.
//!
//! The engine reads source as written text. Written text does not say which
//! type a receiver has, which function a method name reaches, or what that
//! function hands back — so a value's origin stops at the first call into a
//! library. This crate answers those three questions and nothing else.
//!
//! **Why it is a crate of its own.** The answers come from the
//! rust-analyzer libraries, which release weekly as `0.0.x` and promise no
//! API stability. Confining them here keeps that churn out of the engine:
//! the engine takes the answers as an overlay of plain data, exactly as it
//! already takes a resolved-name index, and knows nothing about where they
//! were produced.
//!
//! **What it never does.** It states what the resolver settled and stays
//! silent otherwise. A position the resolver cannot answer for is absent
//! from the overlay, so the engine reaches its own refusal and names the
//! question — rather than being handed a guess that reads like a fact.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the resolver's own dependency graph carries duplicates this crate cannot \
              deduplicate — embedded-io, foldhash, hashbrown, rustc-hash, syn, windows-sys \
              and winnow each arrive at two or three versions through rust-analyzer"
)]

mod error;
mod overlay;
mod workspace;

pub use self::{
    error::ResolveError,
    overlay::{Resolution, ResolvedCall},
    workspace::resolve
};
