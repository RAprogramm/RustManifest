// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub struct ConfigLoader {
    path: std::path::PathBuf
}

pub enum EventDispatcher {
    Idle
}

pub trait RequestHandler {
    fn handle(&self);
}
