// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Turning one proving site into one capability.
//!
//! The effect category alone cannot separate a program that answers a
//! socket from one that dials another program: both are `net`. The path
//! that proved the effect can, because binding a local socket and
//! connecting to a remote one are written with different names. That is why
//! this classification reads the site and not just the category, and why a
//! category it cannot separate produces the weaker of the two answers
//! rather than a guess at the stronger one.

use super::Capability;
use crate::semantic::behavior::{Effect, EffectSite};

/// Path segments that stand for binding a local socket rather than dialling
/// a remote one.
const LISTENING_TYPES: [&str; 2] = ["TcpListener", "UdpSocket"];

/// Final segments that name the act of taking a local socket.
const LISTENING_CALLS: [&str; 2] = ["bind", "listen"];

/// Returns the capability `site` proves, or `None` when it proves none.
///
/// `unsafe` proves no capability: it is a statement about how the code is
/// checked, not about what the program can do to the world.
pub(super) fn of(site: &EffectSite) -> Option<Capability> {
    match site.effect {
        Effect::Unsafe => None,
        Effect::Network => Some(if takes_a_local_socket(&site.path) {
            Capability::Serves
        } else {
            Capability::Fetches
        }),
        Effect::DatabaseRead | Effect::DatabaseWrite => Some(Capability::Stores),
        Effect::Environment => Some(Capability::Configures),
        Effect::FileSystem => Some(Capability::Persists),
        Effect::ConsoleIo => Some(Capability::Reports),
        Effect::Process => Some(Capability::Spawns)
    }
}

/// Returns whether `path` takes a local socket, by naming a listening
/// socket type or ending in the call that binds one.
fn takes_a_local_socket(path: &str) -> bool {
    let segments: Vec<&str> = path.split("::").collect();
    segments
        .iter()
        .any(|segment| LISTENING_TYPES.contains(segment))
        || segments
            .last()
            .is_some_and(|last| LISTENING_CALLS.contains(last))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use rustmanifest_schema::Range;

    use super::*;

    fn site(effect: Effect, path: &str) -> EffectSite {
        EffectSite {
            effect,
            path: path.to_owned(),
            range: Range {
                start: 0, end: 1
            }
        }
    }

    #[test]
    fn binding_a_socket_serves_and_dialling_one_fetches() {
        assert_eq!(
            of(&site(Effect::Network, "std::net::TcpListener::bind")),
            Some(Capability::Serves)
        );
        assert_eq!(
            of(&site(Effect::Network, "std::net::UdpSocket::bind")),
            Some(Capability::Serves)
        );
        assert_eq!(
            of(&site(Effect::Network, "std::net::TcpStream::connect")),
            Some(Capability::Fetches)
        );
    }

    #[test]
    fn a_bind_call_serves_even_when_the_type_is_not_written() {
        assert_eq!(
            of(&site(Effect::Network, "axum::serve::bind")),
            Some(Capability::Serves)
        );
    }

    #[test]
    fn each_remaining_category_maps_to_one_capability() {
        assert_eq!(
            of(&site(Effect::DatabaseRead, "sqlx::query")),
            Some(Capability::Stores)
        );
        assert_eq!(
            of(&site(Effect::DatabaseWrite, "sqlx::query")),
            Some(Capability::Stores)
        );
        assert_eq!(
            of(&site(Effect::Environment, "std::env::var")),
            Some(Capability::Configures)
        );
        assert_eq!(
            of(&site(Effect::FileSystem, "std::fs::read")),
            Some(Capability::Persists)
        );
        assert_eq!(
            of(&site(Effect::ConsoleIo, "println")),
            Some(Capability::Reports)
        );
        assert_eq!(
            of(&site(Effect::Process, "std::process::Command")),
            Some(Capability::Spawns)
        );
    }

    #[test]
    fn unsafe_proves_no_capability() {
        assert_eq!(of(&site(Effect::Unsafe, "unsafe")), None);
    }
}
