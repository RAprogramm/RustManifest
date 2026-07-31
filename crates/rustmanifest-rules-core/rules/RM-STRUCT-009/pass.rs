// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

pub trait Storage {
    fn save(&mut self, key: &str, data: &[u8]);
    fn load(&self, key: &str) -> Option<Vec<u8>>;
}

pub struct FakeStorage {
    data: HashMap<String, Vec<u8>>
}

impl Storage for FakeStorage {
    fn save(&mut self, key: &str, data: &[u8]) {
        self.data.insert(key.to_owned(), data.to_vec());
    }

    fn load(&self, key: &str) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }
}
