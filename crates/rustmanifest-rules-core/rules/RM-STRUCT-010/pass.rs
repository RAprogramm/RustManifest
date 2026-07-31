// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct Timestamps {
    created_at: u64,
    updated_at: u64
}

pub struct User {
    id:         u64,
    timestamps: Timestamps
}

pub struct Post {
    body:       String,
    timestamps: Timestamps
}
