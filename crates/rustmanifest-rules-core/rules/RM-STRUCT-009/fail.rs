// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

use mockall::automock;

#[automock]
pub trait Storage {
    fn save(&mut self, key: &str, data: &[u8]);
    fn load(&self, key: &str) -> Option<Vec<u8>>;
}
