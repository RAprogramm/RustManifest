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

use super::kind::Capability;
use crate::semantic::behavior::{Effect, EffectSite};

/// The library types whose values are a local socket, each named by the
/// item that declares it — two written type names named them before, and
/// named any library's namesake as readily.
const LISTENING_TYPES: [&str; 2] = ["std::net::tcp::TcpListener", "std::net::udp::UdpSocket"];

/// Final segments that name the act of taking a local socket.
const LISTENING_CALLS: [&str; 2] = ["bind", "listen"];

/// The library items that end *this* process rather than start another
/// one, each named by the item itself.
const OWN_EXIT: [&str; 3] = [
    "std::process::ExitCode",
    "std::process::abort",
    "std::process::exit"
];

/// Returns the capability `site` proves, or `None` when it proves none.
///
/// `unsafe` proves no capability: it is a statement about how the code is
/// checked, not about what the program can do to the world.
pub(super) fn of(site: &EffectSite) -> Option<Capability> {
    match site.effect {
        Effect::Unsafe => None,
        Effect::Network => Some(if takes_a_local_socket(&site.proof) {
            Capability::Serves
        } else {
            Capability::Fetches
        }),
        Effect::DatabaseRead | Effect::DatabaseWrite | Effect::Database => {
            Some(Capability::Stores)
        }
        Effect::Environment => Some(Capability::Configures),
        Effect::FileSystem => Some(Capability::Persists),
        Effect::ConsoleIo => Some(Capability::Reports),
        Effect::Process if ends_this_process(&site.proof) => None,
        Effect::Process => Some(Capability::Spawns)
    }
}

/// Returns whether `proof` names this process ending itself rather than a
/// child starting.
///
/// The behavioral tier files everything under `std::process` as one
/// category, so `ExitCode`, `exit` and `abort` arrive alongside `Command`.
/// Reading the proof separates them: leaving is not spawning, and a
/// capability row claiming otherwise would be false. The category itself is
/// left as it is — narrowing it is a change to shared vocabulary, not to
/// this classification.
fn ends_this_process(proof: &str) -> bool {
    OWN_EXIT
        .iter()
        .any(|named| proof == *named || proof.starts_with(&format!("{named}::")))
}

/// Returns whether `path` takes a local socket, by naming a listening
/// socket type or ending in the call that binds one.
fn takes_a_local_socket(path: &str) -> bool {
    LISTENING_TYPES
        .iter()
        .any(|named| path == *named || path.starts_with(&format!("{named}::")))
        || path
            .rsplit("::")
            .next()
            .is_some_and(|last| LISTENING_CALLS.contains(&last))
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
            proof: path.to_owned(),
            range: Range {
                start: 0, end: 1
            }
        }
    }

    #[test]
    fn binding_a_socket_serves_and_dialling_one_fetches() {
        assert_eq!(
            of(&site(Effect::Network, "std::net::tcp::TcpListener::bind")),
            Some(Capability::Serves)
        );
        assert_eq!(
            of(&site(Effect::Network, "std::net::udp::UdpSocket::bind")),
            Some(Capability::Serves)
        );
        assert_eq!(
            of(&site(Effect::Network, "std::net::tcp::TcpStream::connect")),
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
            of(&site(Effect::Process, "std::process::Command::new")),
            Some(Capability::Spawns)
        );
    }

    /// `std::process` covers both starting a child and ending this
    /// process; only the first is a capability to spawn.
    #[test]
    fn leaving_this_process_is_not_spawning_another() {
        assert_eq!(
            of(&site(Effect::Process, "std::process::Command::new")),
            Some(Capability::Spawns)
        );
        assert_eq!(
            of(&site(Effect::Process, "std::process::ExitCode::from")),
            None
        );
        assert_eq!(of(&site(Effect::Process, "std::process::exit")), None);
        assert_eq!(of(&site(Effect::Process, "std::process::abort")), None);
    }

    #[test]
    fn unsafe_proves_no_capability() {
        assert_eq!(of(&site(Effect::Unsafe, "unsafe")), None);
    }
}
