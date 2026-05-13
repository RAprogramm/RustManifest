// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

fn validate_password(input: &str) -> bool {
    !input.is_empty()
}

fn load_api_key_from_env() -> Option<String> {
    std::env::var("API_KEY").ok()
}
