// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Request {
    body: String,
    sent: bool
}

impl Request {
    pub fn body(mut self, body: String) -> Self {
        self.body = body;
        self
    }

    pub fn set_sent(mut self) -> Self {
        self.sent = true;
        self
    }
}
