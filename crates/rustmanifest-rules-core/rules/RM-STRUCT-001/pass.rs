// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Config {
    path: std::path::PathBuf
}

pub struct RequestBuilder {
    url: String
}

pub trait Request {
    fn url(&self) -> &str;
}

impl Config {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            path
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}
