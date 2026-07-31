// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub use crate::widget::Widget;

pub struct Renderer;

pub mod layout {
    pub struct Frame;
}

pub mod widget {
    use crate::{Renderer, layout::Frame};

    pub struct Widget {
        renderer: Renderer,
        frame:    Frame
    }
}
