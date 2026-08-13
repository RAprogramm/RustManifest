// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge pack for the `tokio` crate: asynchronous network sockets
//! and child-process control.
//!
//! Task spawning, timers, and channels stay out: they carry no effect
//! in the engine's taxonomy, and a blanket crate effect would smear
//! network onto pure coordination code. Filesystem helpers need no
//! rows either — `tokio::fs::` paths already classify through the
//! path-pattern sink.

use super::{super::behavior::Effect, CrateKnowledge};

/// What the engine knows about `tokio`.
pub(super) const PACK: CrateKnowledge = CrateKnowledge {
    name:           "tokio",
    method_effects: &[
        ("Child", "kill", Effect::Process),
        ("Child", "start_kill", Effect::Process),
        ("Child", "wait", Effect::Process),
        ("Child", "wait_with_output", Effect::Process),
        ("Command", "output", Effect::Process),
        ("Command", "spawn", Effect::Process),
        ("Command", "status", Effect::Process),
        ("TcpListener", "accept", Effect::Network),
        ("TcpStream", "flush", Effect::Network),
        ("TcpStream", "peek", Effect::Network),
        ("TcpStream", "read", Effect::Network),
        ("TcpStream", "read_exact", Effect::Network),
        ("TcpStream", "read_to_end", Effect::Network),
        ("TcpStream", "read_to_string", Effect::Network),
        ("TcpStream", "readable", Effect::Network),
        ("TcpStream", "shutdown", Effect::Network),
        ("TcpStream", "try_read", Effect::Network),
        ("TcpStream", "try_write", Effect::Network),
        ("TcpStream", "writable", Effect::Network),
        ("TcpStream", "write", Effect::Network),
        ("TcpStream", "write_all", Effect::Network),
        ("UdpSocket", "connect", Effect::Network),
        ("UdpSocket", "recv", Effect::Network),
        ("UdpSocket", "recv_from", Effect::Network),
        ("UdpSocket", "send", Effect::Network),
        ("UdpSocket", "send_to", Effect::Network)
    ],
    crate_effect:   None
};
