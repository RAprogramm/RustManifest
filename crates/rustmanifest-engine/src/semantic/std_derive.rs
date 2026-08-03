// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Knowledge derived from the standard library's own sources.
//!
//! The library ships its source with the toolchain, and it is ordinary
//! code in the same language the engine already reads. Everything the
//! output forms rest on is therefore a fact with a location, not an
//! assertion someone typed into this repository: which internal function
//! a print macro calls, whether that function takes the stream's lock,
//! whether it can panic, and which writer type the stream static holds.
//!
//! A derivation that fails to find its evidence yields nothing. The
//! caller then has no form to judge against and must stay silent, which
//! is the honest outcome — the alternative is exactly the hand-written
//! constant this module exists to remove.

use std::path::{Path, PathBuf};

use rustmanifest_schema::Range;
use syn::spanned::Spanned;

use super::behavior::behavior;
use crate::source::Source;

/// One property of the standard output path, with the exact expression
/// that proves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdFact {
    /// Property name, as the forms refer to it.
    pub property: String,
    /// What the derivation observed, in the library's own vocabulary.
    pub observed: String,
    /// File the evidence lives in.
    pub file:     PathBuf,
    /// Byte range of the proving expression itself.
    pub range:    Range,
    /// One-based line of the proving expression.
    pub line:     usize
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

/// Derives what the print path guarantees, by reading the library's own
/// standard-output module.
///
/// Three properties are derived, each from the body of the internal
/// print function: that it takes the stream lock on every call, that it
/// routes its argument through the formatting machinery, and that it
/// panics when the write fails. The stream's buffering is derived from
/// the writer type the stream static holds.
#[must_use]
pub fn output_knowledge(root: &Path) -> StdOutputKnowledge {
    let stdio = root.join("std/src/io/stdio.rs");
    let Ok(text) = std::fs::read_to_string(&stdio) else {
        return StdOutputKnowledge::default();
    };
    let sources = [Source::new(stdio.clone(), text.clone())];
    let facts = behavior(&sources);
    let mut derived = Vec::new();
    for fact in &facts {
        if fact.function.ends_with("::write_fmt")
            && let Some(range) = expression_of(&text, &fact.function, ExpressionKind::LockCall)
        {
            derived.push(fact_at(
                "lock-per-call",
                &format!(
                    "`{}` locks the stream inside the call it serves",
                    fact.function
                ),
                &stdio,
                &text,
                range
            ));
        }
        if fact.function != "print_to" {
            continue;
        }
        if let Some(range) = expression_of(&text, &fact.function, ExpressionKind::FormatWrite) {
            derived.push(fact_at(
                "format-machinery",
                "the print path writes through the formatting machinery",
                &stdio,
                &text,
                range
            ));
        }
        if let Some(range) = expression_of(&text, &fact.function, ExpressionKind::Panic) {
            derived.push(fact_at(
                "panic-on-failure",
                "the print path panics when the write fails",
                &stdio,
                &text,
                range
            ));
        }
    }
    if let Some(range) = writer_static(&text, "STDOUT", "LineWriter") {
        derived.push(fact_at(
            "line-buffered",
            "the standard output static holds a line-buffering writer",
            &stdio,
            &text,
            range
        ));
    }
    StdOutputKnowledge {
        facts: derived
    }
}

/// Builds one fact, resolving the line the proving expression sits on.
fn fact_at(property: &str, observed: &str, file: &Path, text: &str, range: Range) -> StdFact {
    StdFact {
        property: property.to_owned(),
        observed: observed.to_owned(),
        file: file.to_path_buf(),
        range,
        line: text[..range.start as usize].lines().count() + 1
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

/// Byte range of the proving expression inside the named function, found
/// by walking the parsed body rather than by searching text.
fn expression_of(text: &str, function: &str, kind: ExpressionKind) -> Option<Range> {
    let wanted = function.rsplit("::").next().unwrap_or(function);
    let file = syn::parse_file(text).ok()?;
    let mut items = Vec::new();
    crate::ast::flattened(&file.items, &mut items);
    for item in items {
        let block = match item {
            syn::Item::Fn(declared) if declared.sig.ident == wanted => declared.block.as_ref(),
            syn::Item::Impl(block) => {
                let found = block.items.iter().find_map(|member| match member {
                    syn::ImplItem::Fn(method) if method.sig.ident == wanted => Some(&method.block),
                    _ => None
                });
                match found {
                    Some(body) => body,
                    None => continue
                }
            }
            _ => continue
        };
        let mut scan = Locate {
            kind,
            found: None
        };
        syn::visit::Visit::visit_block(&mut scan, block);
        if let Some(range) = scan.found {
            return Some(range);
        }
    }
    None
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
    fn note(&mut self, span: std::ops::Range<usize>) {
        if self.found.is_none() {
            self.found = Some(Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            });
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
            self.note(call.span().byte_range());
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
            self.note(invocation.span().byte_range());
        }
        syn::visit::visit_macro(self, invocation);
    }
}

/// Byte range of a static's declaration when its declared type names the
/// given writer, read from the parsed item rather than from the text.
fn writer_static(text: &str, name: &str, writer: &str) -> Option<Range> {
    use syn::spanned::Spanned;
    let file = syn::parse_file(text).ok()?;
    let mut items = Vec::new();
    crate::ast::flattened(&file.items, &mut items);
    for item in items {
        let syn::Item::Static(declared) = item else {
            continue;
        };
        if declared.ident != name {
            continue;
        }
        if !type_names(&declared.ty).iter().any(|named| named == writer) {
            continue;
        }
        let span = declared.span().byte_range();
        return Some(Range {
            start: u32::try_from(span.start).unwrap_or(u32::MAX),
            end:   u32::try_from(span.end).unwrap_or(u32::MAX)
        });
    }
    None
}

/// Every type name appearing in a declared type, generic arguments
/// included.
fn type_names(declared: &syn::Type) -> Vec<String> {
    let mut names = Vec::new();
    let syn::Type::Path(path) = declared else {
        return names;
    };
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
            "lock-per-call",
            "format-machinery",
            "panic-on-failure",
            "line-buffered"
        ] {
            let fact = knowledge
                .fact(property)
                .unwrap_or_else(|| panic!("`{property}` must be derived from the library source"));
            assert!(fact.range.end > fact.range.start);
            assert!(fact.file.ends_with("std/src/io/stdio.rs"));
            assert!(!fact.observed.is_empty());
        }
    }
}
