// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The output section of a function card: where the function writes,
//! through what buffering, and under what lock — the measurement the
//! facts export already carries, brought to the cursor unchanged.
//!
//! Only uniform readings are spoken: writes to one stream that disagree
//! about buffering or lock keep the site count alone — an absent clause
//! is a smaller claim, never a vaguer one.

use rustmanifest_engine::semantic::{Acquisition, Buffering, OutputWrite, Stream};

/// The rendered output section, or nothing where no write is recorded.
pub(in crate::server) fn output_section(writes: &[OutputWrite]) -> Option<String> {
    if writes.is_empty() {
        return None;
    }
    let mut streams: Vec<Stream> = Vec::new();
    for write in writes {
        if !streams.contains(&write.stream) {
            streams.push(write.stream);
        }
    }
    let told: Vec<String> = streams
        .iter()
        .map(|stream| spoken(*stream, writes))
        .collect();
    Some(format!("**output** — {}", told.join(" · ")))
}

/// One stream's claim: the site count, with the buffering and lock
/// clauses where every site of the stream agrees on them.
fn spoken(stream: Stream, writes: &[OutputWrite]) -> String {
    let sites: Vec<&OutputWrite> = writes
        .iter()
        .filter(|write| write.stream == stream)
        .collect();
    let mut clause = format!("{}: {} write(s)", named(stream), sites.len());
    if sites
        .iter()
        .all(|write| write.buffering == Buffering::Block)
    {
        clause.push_str(" through a block buffer");
    } else if sites
        .iter()
        .all(|write| write.buffering == Buffering::StreamOwn)
    {
        clause.push_str(" with the stream's own buffering");
    }
    if sites
        .iter()
        .all(|write| write.acquisition == Acquisition::AtSite)
    {
        clause.push_str(", lock taken at the site");
    } else if sites
        .iter()
        .all(|write| write.acquisition == Acquisition::Held)
    {
        clause.push_str(", lock already held");
    }
    clause
}

/// The stream's name on a card.
const fn named(stream: Stream) -> &'static str {
    match stream {
        Stream::Stdout => "stdout",
        Stream::Stderr => "stderr",
        Stream::CallerSupplied => "a caller-supplied writer",
        Stream::Unknown => "an unproven destination"
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use rustmanifest_engine::{Source, behavior};

    /// The rendered section of one written snippet's first function.
    fn section(text: &str) -> Option<String> {
        let facts = behavior(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]);
        super::output_section(&facts[0].output_writes)
    }

    /// A print macro writes stdout with the stream's own buffering and
    /// the lock taken at the site — and the card says exactly that.
    #[test]
    fn a_print_writes_stdout_with_its_own_buffering() {
        let told = section("fn shout() { println!(\"hi\"); }").unwrap();
        assert!(told.contains("stdout: 1 write(s)"), "{told}");
        assert!(told.contains("the stream's own buffering"), "{told}");
        assert!(told.contains("lock taken at the site"), "{told}");
    }

    /// A write bound through a buffering wrapper names the block buffer
    /// the destination doctrine proved.
    #[test]
    fn a_buffered_write_names_the_block_buffer() {
        let told = section(
            "use std::io::Write;\nfn dump() {\n    let out = std::io::stdout();\n    let mut \
             held = std::io::BufWriter::new(out.lock());\n    held.write_all(b\"hi\").ok();\n}"
        )
        .unwrap();
        assert!(told.contains("stdout"), "{told}");
        assert!(told.contains("through a block buffer"), "{told}");
    }

    /// A function without writes claims nothing: the section is absent.
    #[test]
    fn a_function_without_writes_has_no_section() {
        assert!(section("fn calc(value: u64) -> u64 { value * 2 }").is_none());
    }
}
