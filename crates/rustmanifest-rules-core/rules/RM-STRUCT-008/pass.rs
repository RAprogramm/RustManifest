// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

const BUFFER_SIZE: usize = 8192;

pub struct Pool {
    size: usize
}

impl Pool {
    pub const MAX_SIZE: usize = 100;

    pub fn new(size: usize) -> Self {
        Self {
            size
        }
    }

    pub fn buffered(&self) -> usize {
        self.size * BUFFER_SIZE
    }
}
