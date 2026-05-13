// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

use std::process::Command;

fn list(path: &str) -> std::io::Result<std::process::Output> {
    Command::new("ls").arg("-la").arg(path).output()
}
