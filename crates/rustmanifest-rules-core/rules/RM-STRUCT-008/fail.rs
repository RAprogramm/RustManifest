// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub const MAX_CONNECTIONS: usize = 100;

pub struct Pool {
    size: usize
}

impl Pool {
    pub fn size(&self) -> usize {
        self.size
    }
}
