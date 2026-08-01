// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture: this file stands for a module whose author forgot the
//! `mod handlers;` line in the parent — handlers exist but the
//! compiler never sees them. The check needs a multi-file workspace,
//! so the real coverage lives in the engine unit tests.

pub fn on_click() {}
