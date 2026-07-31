// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Config {
    content: String
}

impl Config {
    pub fn new(path: &str) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self {
            content
        })
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}
