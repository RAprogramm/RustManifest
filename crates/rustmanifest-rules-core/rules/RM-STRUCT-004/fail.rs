// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Document {
    content: String
}

impl Document {
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn length(&self) -> usize {
        self.content.len()
    }

    pub fn save(&self) {}

    pub fn publish(&self) {}

    pub fn validate(&self) -> bool {
        !self.content.is_empty()
    }

    pub fn render_html(&self) -> String {
        self.content.clone()
    }
}
