// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

fn lookup(id: i64) -> String {
    format!("SELECT * FROM users WHERE id = {id}")
}
