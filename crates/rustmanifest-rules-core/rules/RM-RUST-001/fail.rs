// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

fn parse_port(raw: &str) -> u16 {
    raw.parse().unwrap()
}

fn must_have(name: &str) -> String {
    std::env::var(name).expect("env var missing")
}
