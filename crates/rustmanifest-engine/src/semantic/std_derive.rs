// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge derived from the standard library's own sources.
//!
//! The library ships its source with the toolchain, and it is ordinary
//! code in the same language the engine already reads. Everything the
//! output forms rest on is therefore a fact with a location, not an
//! assertion someone typed into this repository: which internal function
//! a print macro calls, whether that function takes the stream's lock,
//! whether it can panic, and which writer chain each stream's handle
//! declares.
//!
//! Every fact carries the library's own text at the range it was read
//! from, so a consumer quotes the evidence instead of paraphrasing it.
//! The sentence around the quote is this engine's wording, but every
//! name in it — the function, the method, the writer types — was read
//! from the source, and the quote is what proves it.
//!
//! Properties that differ between the two standard streams are named per
//! stream, `property:stdout` and `property:stderr`, because the library
//! gives the two streams different handles with different writer chains
//! and an independent lock each. A property derived from one stream's
//! declaration says nothing about the other, and an unqualified name
//! would hide exactly that.
//!
//! A derivation that fails to find its evidence yields nothing. The
//! caller then has no form to judge against and must stay silent, which
//! is the honest outcome — the alternative is exactly the hand-written
//! constant this module exists to remove.

use std::path::{Path, PathBuf};

use rustmanifest_schema::Range;
use syn::spanned::Spanned;

use super::{behavior::behavior, expr::impl_type_name};
use crate::source::Source;

/// One property of the standard output path, with the exact expression
/// that proves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdFact {
    /// Property name, as the forms refer to it. A property that holds
    /// per stream carries the stream after a colon.
    pub property: String,
    /// What the derivation read, spelled with the names it read there.
    pub observed: String,
    /// File the evidence lives in.
    pub file:     PathBuf,
    /// Byte range of the proving expression itself.
    pub range:    Range,
    /// The library's own documentation sentence for this property, read
    /// from the doc comment of the item that declares it. `None` when
    /// the library documents nothing about it, which is stated rather
    /// than filled in.
    pub doc:      Option<DocQuote>,
    /// One-based line of the proving expression.
    pub line:     usize,
    /// The library's own text at [`StdFact::range`], with runs of
    /// whitespace collapsed onto one line. This is the evidence a trace
    /// step shows: the source itself, never a restatement of it.
    pub snippet:  String
}

/// One sentence the library writes about itself, with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocQuote {
    /// The sentence, as written in the library's doc comment.
    pub text: String,
    /// File the sentence lives in.
    pub file: PathBuf,
    /// One-based line of the sentence.
    pub line: usize
}

/// Everything derived about the print path of the active toolchain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StdOutputKnowledge {
    /// Facts in derivation order.
    pub facts: Vec<StdFact>
}

impl StdOutputKnowledge {
    /// Whether a property was derived.
    #[must_use]
    pub fn has(&self, property: &str) -> bool {
        self.facts.iter().any(|fact| fact.property == property)
    }

    /// The fact for one property, when derived.
    #[must_use]
    pub fn fact(&self, property: &str) -> Option<&StdFact> {
        self.facts.iter().find(|fact| fact.property == property)
    }
}

/// Path of the standard library's source tree for the active toolchain,
/// when the component is installed.
#[must_use]
pub fn std_source_root() -> Option<PathBuf> {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .ok()?;
    let sysroot = String::from_utf8(output.stdout).ok()?;
    let root = Path::new(sysroot.trim()).join("lib/rustlib/src/rust/library");
    root.is_dir().then_some(root)
}

/// The two standard streams: the handle type the library declares for
/// each one, and the name a per-stream property carries.
const HANDLES: [(&str, &str); 2] = [("Stdout", "stdout"), ("Stderr", "stderr")];

/// Internal function every print macro funnels through, generic over the
/// stream it is handed, which is why the properties read out of it hold
/// for both streams and carry no stream in their name.
const PRINT_FUNCTION: &str = "print_to";

/// Writer types this derivation recognizes as holding a buffer, with the
/// discipline each one implements.
const BUFFERING_WRITERS: [(&str, &str); 2] = [
    ("LineWriter", "line-buffered"),
    ("BufWriter", "block-buffered")
];

/// Types that hand their bytes straight to the operating system handle.
/// A chain ending in one of them, naming no buffering writer on the way,
/// buffers nothing.
const RAW_HANDLES: [&str; 2] = ["StdoutRaw", "StderrRaw"];

/// Derives what the print path guarantees, by reading the library's own
/// standard-output module.
///
/// Per stream: that the handle takes its own lock inside the call that
/// writes through it, and which writer chain the handle declares. Shared
/// between the streams: that the print path routes its argument through
/// the formatting machinery, and that it panics when the write fails —
/// both read out of the one generic function both streams reach.
#[must_use]
pub fn output_knowledge(root: &Path) -> StdOutputKnowledge {
    let stdio = root.join("std/src/io/stdio.rs");
    let Ok(text) = std::fs::read_to_string(&stdio) else {
        return StdOutputKnowledge::default();
    };
    let Ok(parsed) = syn::parse_file(&text) else {
        return StdOutputKnowledge::default();
    };
    let mut items = Vec::new();
    crate::ast::flattened(&parsed.items, &mut items);
    let sources = [Source::new(stdio.clone(), text.clone())];
    let behavior_facts = behavior(&sources);
    let library = Evidence {
        file:  &stdio,
        text:  &text,
        items: &items
    };
    let mut derived = Vec::new();
    for (handle, stream) in HANDLES {
        let writes = format!("{handle}::write_fmt");
        if behavior_facts.iter().any(|fact| fact.function == writes)
            && let Some(range) =
                library.method_expression(handle, "write_fmt", ExpressionKind::LockCall)
        {
            derived.push(library.fact(
                &format!("lock-per-call:{stream}"),
                &format!("`{writes}` takes the {stream} lock inside the call it serves"),
                range
            ));
        }
        if let Some(fact) = library.buffering(handle, stream) {
            derived.push(fact);
        }
    }
    if behavior_facts
        .iter()
        .any(|fact| fact.function == PRINT_FUNCTION)
    {
        if let Some(range) = library.free_expression(PRINT_FUNCTION, ExpressionKind::FormatWrite) {
            derived.push(library.fact(
                "format-machinery",
                &format!("`{PRINT_FUNCTION}` writes through the formatting machinery"),
                range
            ));
        }
        if let Some(range) = library.free_expression(PRINT_FUNCTION, ExpressionKind::Panic) {
            derived.push(library.fact(
                "panic-on-failure",
                &format!("`{PRINT_FUNCTION}` panics when the write fails"),
                range
            ));
        }
    }
    for fact in &mut derived {
        if let Some((_, file, item, keyword)) = DOC_SOURCES
            .iter()
            .find(|(property, _, _, _)| *property == fact.property)
        {
            fact.doc = doc_quote(root, file, item, keyword);
        }
    }
    StdOutputKnowledge {
        facts: derived
    }
}

/// Where each property's own documentation sentence lives in the
/// library, and the word that identifies the sentence inside it.
///
/// The library documents its own guarantees; a finding that cites the
/// code without the sentence beside it leaves the reader to take the
/// analyzer's word for what the code means. Both are quoted, and a
/// property whose sentence cannot be found carries none rather than a
/// substitute written here.
const DOC_SOURCES: [(&str, &str, &str, &str); 4] = [
    (
        "panic-on-failure",
        "std/src/macros.rs",
        "println",
        "Panics if writing"
    ),
    ("format-machinery", "std/src/macros.rs", "println", "format"),
    (
        "lock-per-call:stdout",
        "std/src/io/stdio.rs",
        "Stdout",
        "lock"
    ),
    (
        "lock-per-call:stderr",
        "std/src/io/stdio.rs",
        "Stderr",
        "lock"
    )
];

/// Reads the library's own sentence for one property.
///
/// The item is located by name in the named file and its doc comment is
/// searched for the first sentence carrying the identifying word. A file
/// that does not parse, an item that is absent, or a doc comment that
/// never mentions the word all yield nothing.
fn doc_quote(root: &Path, file: &str, item_name: &str, keyword: &str) -> Option<DocQuote> {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed = syn::parse_file(&text).ok()?;
    let mut items = Vec::new();
    crate::ast::flattened(&parsed.items, &mut items);
    for item in items {
        let (attrs, name) = match item {
            syn::Item::Macro(declared) => (
                &declared.attrs,
                declared
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            ),
            syn::Item::Struct(declared) => (&declared.attrs, declared.ident.to_string()),
            syn::Item::Fn(declared) => (&declared.attrs, declared.sig.ident.to_string()),
            _ => continue
        };
        if name != item_name {
            continue;
        }
        if let Some(quote) = doc_sentence(attrs, keyword, &text, &path) {
            return Some(quote);
        }
    }
    None
}

/// First doc line carrying the identifying word, with its location.
fn doc_sentence(
    attrs: &[syn::Attribute],
    keyword: &str,
    text: &str,
    path: &Path
) -> Option<DocQuote> {
    for attribute in attrs {
        if !attribute.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(pair) = &attribute.meta else {
            continue;
        };
        let syn::Expr::Lit(literal) = &pair.value else {
            continue;
        };
        let syn::Lit::Str(line) = &literal.lit else {
            continue;
        };
        let sentence = line.value();
        let trimmed = sentence.trim();
        if trimmed.is_empty() || !trimmed.contains(keyword) {
            continue;
        }
        let located = text.find(trimmed).map_or(1, |offset| {
            text.get(..offset)
                .map_or(1, |before| before.matches('\n').count() + 1)
        });
        return Some(DocQuote {
            text: trimmed.to_owned(),
            file: path.to_path_buf(),
            line: located
        });
    }
    None
}

/// The library file under derivation, parsed once and read from many
/// times.
struct Evidence<'a> {
    /// Path the facts cite.
    file:  &'a Path,
    /// Full text, the only source of a quoted snippet.
    text:  &'a str,
    /// Every item of the file, modules flattened.
    items: &'a [&'a syn::Item]
}

impl Evidence<'_> {
    /// Builds one fact, resolving the line and quoting the library's own
    /// text at the proving range.
    fn fact(&self, property: &str, observed: &str, range: Range) -> StdFact {
        StdFact {
            property: property.to_owned(),
            observed: observed.to_owned(),
            file: self.file.to_path_buf(),
            range,
            line: self
                .text
                .get(..range.start as usize)
                .map_or(1, |before| before.matches('\n').count() + 1),
            snippet: quoted(self.text, range),
            doc: None
        }
    }

    /// Buffering of one stream, read from the writer chain its handle
    /// type declares. A chain naming a writer this derivation does not
    /// recognize yields nothing at all rather than a guess.
    fn buffering(&self, handle: &str, stream: &str) -> Option<StdFact> {
        let (chain, range) = self.writer_chain(handle)?;
        let kind = buffering_kind(&chain)?;
        Some(self.fact(
            &format!("{kind}:{stream}"),
            &format!(
                "the `{handle}` handle declares the writer chain `{}`",
                chain.join(" -> ")
            ),
            range
        ))
    }

    /// Type names the handle's own field declares, outermost first, with
    /// the byte range of that declaration.
    fn writer_chain(&self, handle: &str) -> Option<(Vec<String>, Range)> {
        for item in self.items {
            let syn::Item::Struct(declared) = item else {
                continue;
            };
            if declared.ident != handle {
                continue;
            }
            for field in &declared.fields {
                let names = type_names(&field.ty);
                if names.is_empty() {
                    continue;
                }
                return Some((names, byte_range(&field.span().byte_range())));
            }
        }
        None
    }

    /// Byte range of a proving expression inside a free function's body.
    fn free_expression(&self, function: &str, kind: ExpressionKind) -> Option<Range> {
        for item in self.items {
            let syn::Item::Fn(declared) = item else {
                continue;
            };
            if declared.sig.ident != function {
                continue;
            }
            if let Some(range) = locate(&declared.block, kind) {
                return Some(range);
            }
        }
        None
    }

    /// Byte range of a proving expression inside a method written for
    /// the named type, across every implementation block that type has:
    /// the library splits the write trait between a handle and a borrow
    /// of it, and the proving call sits in the borrow's block.
    fn method_expression(&self, owner: &str, method: &str, kind: ExpressionKind) -> Option<Range> {
        for item in self.items {
            let syn::Item::Impl(block) = item else {
                continue;
            };
            if impl_type_name(block).as_deref() != Some(owner) {
                continue;
            }
            for member in &block.items {
                let syn::ImplItem::Fn(declared) = member else {
                    continue;
                };
                if declared.sig.ident != method {
                    continue;
                }
                if let Some(range) = locate(&declared.block, kind) {
                    return Some(range);
                }
            }
        }
        None
    }
}

/// Buffering discipline a writer chain implements. A chain naming no
/// recognized writer and ending outside the raw handles yields `None`:
/// an unknown writer is reported as no knowledge, never as no buffer.
fn buffering_kind(chain: &[String]) -> Option<&'static str> {
    if let Some((_, kind)) = BUFFERING_WRITERS
        .iter()
        .find(|(writer, _)| chain.iter().any(|name| name == writer))
    {
        return Some(kind);
    }
    let last = chain.last()?;
    RAW_HANDLES.contains(&last.as_str()).then_some("unbuffered")
}

/// The library's own text at a range, with runs of whitespace collapsed
/// so one quote stays one line.
fn quoted(text: &str, range: Range) -> String {
    text.get(range.start as usize..range.end as usize)
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// A span's byte range, clamped to the width the schema records.
fn byte_range(span: &std::ops::Range<usize>) -> Range {
    Range {
        start: u32::try_from(span.start).unwrap_or(u32::MAX),
        end:   u32::try_from(span.end).unwrap_or(u32::MAX)
    }
}

/// Which proving expression to locate inside a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionKind {
    /// A method call named `lock`.
    LockCall,
    /// A method call named `write_fmt`.
    FormatWrite,
    /// A panic-family macro invocation.
    Panic
}

/// First proving expression of one kind inside a body.
fn locate(block: &syn::Block, kind: ExpressionKind) -> Option<Range> {
    let mut scan = Locate {
        kind,
        found: None
    };
    syn::visit::Visit::visit_block(&mut scan, block);
    scan.found
}

/// Visitor locating one proving expression.
struct Locate {
    /// Expression being looked for.
    kind:  ExpressionKind,
    /// Byte range of the first match.
    found: Option<Range>
}

impl Locate {
    /// Records the first match only.
    fn note(&mut self, span: &std::ops::Range<usize>) {
        if self.found.is_none() {
            self.found = Some(byte_range(span));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for Locate {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let wanted = match self.kind {
            ExpressionKind::LockCall => "lock",
            ExpressionKind::FormatWrite => "write_fmt",
            ExpressionKind::Panic => ""
        };
        if !wanted.is_empty() && call.method == wanted {
            self.note(&call.span().byte_range());
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if self.kind == ExpressionKind::Panic
            && invocation
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "panic")
        {
            self.note(&invocation.span().byte_range());
        }
        syn::visit::visit_macro(self, invocation);
    }
}

/// Every type name appearing in a declared type, outermost first,
/// looking through references, parentheses and groups so a handle field
/// written as a borrow still yields its writer chain.
fn type_names(declared: &syn::Type) -> Vec<String> {
    let mut names = Vec::new();
    match declared {
        syn::Type::Reference(borrowed) => return type_names(&borrowed.elem),
        syn::Type::Paren(inner) => return type_names(&inner.elem),
        syn::Type::Group(inner) => return type_names(&inner.elem),
        syn::Type::Path(path) => {
            for segment in &path.path.segments {
                names.push(segment.ident.to_string());
                if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let syn::GenericArgument::Type(inner) = argument {
                            names.extend(type_names(inner));
                        }
                    }
                }
            }
        }
        _ => ()
    }
    names
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{output_knowledge, std_source_root};

    #[test]
    fn the_print_path_properties_come_from_the_library_itself() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        for property in [
            "lock-per-call:stdout",
            "lock-per-call:stderr",
            "format-machinery",
            "panic-on-failure",
            "line-buffered:stdout",
            "unbuffered:stderr"
        ] {
            let fact = knowledge
                .fact(property)
                .unwrap_or_else(|| panic!("`{property}` must be derived from the library source"));
            assert!(fact.range.end > fact.range.start);
            assert!(fact.file.ends_with("std/src/io/stdio.rs"));
            assert!(!fact.observed.is_empty());
        }
    }

    /// Each stream's buffering must be derived from that stream's own
    /// declaration and registered under that stream's name, so a form can
    /// never carry one stream's evidence to the other.
    #[test]
    fn buffering_is_derived_per_stream_and_never_shared() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        assert!(!knowledge.has("line-buffered"));
        assert!(!knowledge.has("lock-per-call"));
        assert!(!knowledge.has("line-buffered:stderr"));
        assert!(!knowledge.has("unbuffered:stdout"));

        let stdout = knowledge.fact("line-buffered:stdout").unwrap();
        assert!(stdout.snippet.contains("LineWriter"));
        let stderr = knowledge.fact("unbuffered:stderr").unwrap();
        assert!(stderr.snippet.contains("StderrRaw"));
        assert!(!stderr.snippet.contains("LineWriter"));
        assert_ne!(stdout.range, stderr.range);
    }

    /// Every fact quotes the library's own text at the range it names, so
    /// a trace step shows the source and not a sentence from this
    /// repository.
    #[test]
    fn every_fact_quotes_the_source_text_at_its_own_range() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        assert!(!knowledge.facts.is_empty());
        for fact in &knowledge.facts {
            let text = std::fs::read_to_string(&fact.file).unwrap();
            let quoted = super::quoted(&text, fact.range);
            assert_eq!(fact.snippet, quoted);
            assert!(!fact.snippet.is_empty());
            assert!(!fact.snippet.contains('\n'));
            let line = text
                .lines()
                .nth(fact.line - 1)
                .expect("the recorded line exists in the file");
            let head = fact
                .snippet
                .split(' ')
                .next()
                .expect("a quote has a first token");
            assert!(
                line.contains(head),
                "`{}` must start on line {}: {line}",
                fact.snippet,
                fact.line
            );
        }
    }

    /// The two lock facts must come from two different places: one per
    /// handle, because the library gives each stream its own lock.
    #[test]
    fn each_stream_proves_its_own_lock() {
        let Some(root) = std_source_root() else {
            return;
        };
        let knowledge = output_knowledge(&root);
        let stdout = knowledge.fact("lock-per-call:stdout").unwrap();
        let stderr = knowledge.fact("lock-per-call:stderr").unwrap();
        assert_ne!(stdout.range, stderr.range);
    }

    #[test]
    fn a_writer_chain_is_read_through_a_borrow_and_judged_by_its_own_names() {
        let declared: syn::Type =
            syn::parse_str("&'static ReentrantLock<RefCell<LineWriter<StdoutRaw>>>").unwrap();
        assert_eq!(
            super::type_names(&declared),
            ["ReentrantLock", "RefCell", "LineWriter", "StdoutRaw"]
        );
        assert_eq!(
            super::buffering_kind(&super::type_names(&declared)),
            Some("line-buffered")
        );

        let raw: syn::Type = syn::parse_str("&'static ReentrantLock<RefCell<StderrRaw>>").unwrap();
        assert_eq!(
            super::buffering_kind(&super::type_names(&raw)),
            Some("unbuffered")
        );

        let unknown: syn::Type =
            syn::parse_str("&'static ReentrantLock<SomeFutureWriter>").unwrap();
        assert_eq!(super::buffering_kind(&super::type_names(&unknown)), None);
    }
}
