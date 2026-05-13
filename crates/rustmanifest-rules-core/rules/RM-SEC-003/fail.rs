// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

use std::process::Command;

fn remove(path: &str) -> std::io::Result<std::process::Output> {
    Command::new("sh").arg(format!("rm {path}")).output()
}
