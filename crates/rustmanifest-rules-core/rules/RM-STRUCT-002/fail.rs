// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Document {
    title: String
}

impl Document {
    pub fn get_title(&self) -> &str {
        &self.title
    }
}
