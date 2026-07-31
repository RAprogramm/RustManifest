// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Config {
    path: std::path::PathBuf
}

impl Config {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            path
        }
    }

    pub fn content(&self) -> Result<String, std::io::Error> {
        std::fs::read_to_string(&self.path)
    }
}
