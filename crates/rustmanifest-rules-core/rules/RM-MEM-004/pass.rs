// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

fn read(value: &String) {}

fn pass(value: String) -> String {
    value
}

fn main() {
    let s = String::from("hello");
    read(&s);
    println!("{s}");
    let s = pass(s);
    println!("{s}");
}
