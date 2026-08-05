// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which effect a standard-library method performs when called on an
//! instance.
//!
//! The axiom floor under every derived effect chain. Path patterns catch
//! associated calls like `File::open`; these rows catch the instance
//! calls a path pattern cannot see.

use crate::semantic::behavior::Effect;

/// `(type, method, effect)` rows for std methods that perform the
/// effect when called on an instance — the axiom floor under derived
/// effect chains. Path-pattern sinks catch associated calls like
/// `File::open`; these rows catch the instance calls path patterns
/// cannot see. Only rows the std documentation guarantees; accessors
/// stay out.
pub(in crate::semantic) const METHOD_EFFECTS: [(&str, &str, Effect); 47] = [
    ("Child", "kill", Effect::Process),
    ("Child", "wait", Effect::Process),
    ("Child", "wait_with_output", Effect::Process),
    ("Command", "output", Effect::Process),
    ("Command", "spawn", Effect::Process),
    ("Command", "status", Effect::Process),
    ("DirBuilder", "create", Effect::FileSystem),
    ("File", "flush", Effect::FileSystem),
    ("File", "metadata", Effect::FileSystem),
    ("File", "read", Effect::FileSystem),
    ("File", "read_exact", Effect::FileSystem),
    ("File", "read_to_end", Effect::FileSystem),
    ("File", "read_to_string", Effect::FileSystem),
    ("File", "set_len", Effect::FileSystem),
    ("File", "set_permissions", Effect::FileSystem),
    ("File", "sync_all", Effect::FileSystem),
    ("File", "sync_data", Effect::FileSystem),
    ("File", "try_clone", Effect::FileSystem),
    ("File", "write", Effect::FileSystem),
    ("File", "write_all", Effect::FileSystem),
    ("OpenOptions", "open", Effect::FileSystem),
    ("Stderr", "flush", Effect::ConsoleIo),
    ("Stderr", "write", Effect::ConsoleIo),
    ("Stderr", "write_all", Effect::ConsoleIo),
    ("Stdin", "read_line", Effect::ConsoleIo),
    ("Stdin", "read_to_string", Effect::ConsoleIo),
    ("Stdout", "flush", Effect::ConsoleIo),
    ("Stdout", "write", Effect::ConsoleIo),
    ("Stdout", "write_all", Effect::ConsoleIo),
    ("TcpListener", "accept", Effect::Network),
    ("TcpListener", "incoming", Effect::Network),
    ("TcpStream", "flush", Effect::Network),
    ("TcpStream", "peek", Effect::Network),
    ("TcpStream", "read", Effect::Network),
    ("TcpStream", "read_exact", Effect::Network),
    ("TcpStream", "read_to_end", Effect::Network),
    ("TcpStream", "read_to_string", Effect::Network),
    ("TcpStream", "shutdown", Effect::Network),
    ("TcpStream", "write", Effect::Network),
    ("TcpStream", "write_all", Effect::Network),
    ("UdpSocket", "connect", Effect::Network),
    ("UdpSocket", "peek", Effect::Network),
    ("UdpSocket", "peek_from", Effect::Network),
    ("UdpSocket", "recv", Effect::Network),
    ("UdpSocket", "recv_from", Effect::Network),
    ("UdpSocket", "send", Effect::Network),
    ("UdpSocket", "send_to", Effect::Network)
];

/// Looks up the effect a std method performs on its receiver type.
pub(in crate::semantic) fn method_effect(owner: &str, method: &str) -> Option<Effect> {
    METHOD_EFFECTS
        .iter()
        .find(|(known_owner, known_method, _)| *known_owner == owner && *known_method == method)
        .map(|(_, _, effect)| *effect)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{Effect, METHOD_EFFECTS, method_effect};

    #[test]
    fn std_method_axioms_cover_io_and_spare_accessors() {
        assert_eq!(method_effect("File", "write_all"), Some(Effect::FileSystem));
        assert_eq!(method_effect("TcpStream", "read"), Some(Effect::Network));
        assert_eq!(method_effect("Command", "spawn"), Some(Effect::Process));
        assert_eq!(method_effect("Stdout", "flush"), Some(Effect::ConsoleIo));
        assert_eq!(method_effect("File", "path"), None);
        assert_eq!(method_effect("Vec", "push"), None);
    }

    #[test]
    fn the_table_stays_sorted_so_a_reviewer_can_read_it() {
        let rows: Vec<(&str, &str)> = METHOD_EFFECTS
            .iter()
            .map(|(owner, method, _)| (*owner, *method))
            .collect();
        let mut sorted = rows.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, rows);
    }
}
