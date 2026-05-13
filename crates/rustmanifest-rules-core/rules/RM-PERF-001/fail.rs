// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

fn build_ids(count: usize) -> Vec<u64> {
    let mut ids: Vec<u64> = Vec::new();
    for i in 0..count {
        ids.push(i as u64);
    }
    ids
}
