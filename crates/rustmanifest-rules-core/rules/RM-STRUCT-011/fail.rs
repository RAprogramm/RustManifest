// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

pub mod parser {
    use crate::renderer::Theme;

    pub struct Token;

    pub struct Palette {
        theme: Theme
    }
}

pub mod renderer {
    use crate::parser::Token;

    pub struct Theme;

    pub struct Screen {
        cursor: Token
    }
}
