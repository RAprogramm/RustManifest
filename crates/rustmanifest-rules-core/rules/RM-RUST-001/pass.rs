// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

#[derive(Debug)]
struct ConfigError;

fn parse_port(raw: &str) -> Result<u16, ConfigError> {
    raw.parse().map_err(|_| ConfigError)
}

fn must_have(name: &str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError)
}
