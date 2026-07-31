// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Server {
    port:   u16,
    secure: bool
}

impl Server {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            secure: false
        }
    }

    pub fn secure(port: u16) -> Self {
        Self {
            port,
            secure: true
        }
    }
}
