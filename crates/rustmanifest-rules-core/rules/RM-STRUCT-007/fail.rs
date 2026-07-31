// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Request {
    body: String
}

impl Request {
    pub fn set_body(&mut self, body: String) {
        self.body = body;
    }
}
