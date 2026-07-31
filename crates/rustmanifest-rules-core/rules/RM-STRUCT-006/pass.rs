// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Server {
    port:   u16,
    secure: bool
}

impl Server {
    pub fn with_config(port: u16, secure: bool) -> Self {
        Self {
            port,
            secure
        }
    }

    pub fn new(port: u16) -> Self {
        Self::with_config(port, false)
    }

    pub fn secure(port: u16) -> Self {
        Self::with_config(port, true)
    }
}
