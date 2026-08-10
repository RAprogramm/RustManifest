// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fix synthesis for the write-in-a-loop finding: the locked handle
//! wrapped in a block buffer, the form the standard library still asks
//! authors to write themselves — stdout stays line-buffered even off a
//! terminal, so a loop of lines pays a syscall per line until a
//! `BufWriter` stands in front of it.
//!
//! The synthesis is gated hard; all of the following must hold,
//! otherwise no fix is produced and the finding stays a suggestion:
//!
//! - the flagged write goes through a named handle (`writeln!(name, …)`), not a
//!   bare `println!`-family macro, whose rewrite is a different operation;
//! - the handle is bound in the same function as `let name = <expr>.lock();`,
//!   so one edit to the initializer buffers every write through it;
//! - every use of the name in the function is a write-family use — a
//!   `write!`/`writeln!` first argument, or a `flush`/`write_all`/ `write_fmt`
//!   receiver — so the type change stays invisible;
//! - a written `name.flush()` exists: a buffer that only drains on drop
//!   swallows its errors, and the fix must not introduce that.
//!
//! The one edit wraps the initializer in `std::io::BufWriter::new(…)`.
//! It is marked as needing review: buffering changes when output
//! becomes visible, a judgment the facts do not carry.

use proc_macro2::{TokenStream, TokenTree};
use rustmanifest_schema::{Applicability, Fix, Range};
use syn::visit::Visit;

use crate::source::Source;

/// Builds the buffering fix for the write at `site` in `source`, or
/// nothing where safety cannot be proven.
pub(super) fn buffer_fixes(source: &Source, site: Range) -> Vec<Fix> {
    let text = source.text();
    let Ok(parsed) = syn::parse_file(text) else {
        return Vec::new();
    };
    let Some(name) = handle_name(text, site) else {
        return Vec::new();
    };
    let mut watcher = Watcher {
        name,
        site,
        init: None,
        flushed: false,
        forbidden: 0,
        inside: false
    };
    watcher.visit_file(&parsed);
    let Some(init) = watcher.init else {
        return Vec::new();
    };
    if watcher.forbidden > 0 || !watcher.flushed {
        return Vec::new();
    }
    let Some(written) = text.get(init.start as usize..init.end as usize) else {
        return Vec::new();
    };
    vec![Fix {
        file:          None,
        rename_to:     None,
        range:         init,
        replacement:   format!("std::io::BufWriter::new({written})"),
        applicability: Applicability::MaybeIncorrect
    }]
}

/// The identifier the flagged write hands its bytes to: the first token
/// of the write-family macro at `site`, where it is an identifier.
fn handle_name(text: &str, site: Range) -> Option<String> {
    let written = text.get(site.start as usize..site.end as usize)?;
    let opened = written.find('(').or_else(|| written.find('['))?;
    let inner: TokenStream = written
        .get(opened + 1..written.len().saturating_sub(1))?
        .parse()
        .ok()?;
    match inner.into_iter().next()? {
        TokenTree::Ident(ident) => Some(ident.to_string()),
        _ => None
    }
}

/// Walks the function holding the site: finds the `.lock()` binding of
/// the handle, proves every other use is a write-family use, and sees
/// the written flush.
struct Watcher {
    name:      String,
    site:      Range,
    init:      Option<Range>,
    flushed:   bool,
    forbidden: u32,
    inside:    bool
}

impl<'ast> Visit<'ast> for Watcher {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.enter(syn::spanned::Spanned::span(item), |walk| {
            syn::visit::visit_item_fn(walk, item);
        });
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.enter(syn::spanned::Spanned::span(item), |walk| {
            syn::visit::visit_impl_item_fn(walk, item);
        });
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if self.inside
            && let syn::Pat::Ident(pattern) = &local.pat
            && pattern.ident == self.name.as_str()
            && let Some(init) = &local.init
        {
            if locking(&init.expr) {
                self.init = Some(ranged(syn::spanned::Spanned::span(&init.expr)));
            }
            return;
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, called: &'ast syn::ExprMethodCall) {
        if self.inside
            && let syn::Expr::Path(path) = called.receiver.as_ref()
            && path.path.is_ident(&self.name)
        {
            if called.method == "flush" {
                self.flushed = true;
            }
            if !matches!(
                called.method.to_string().as_str(),
                "flush" | "write_all" | "write_fmt" | "write"
            ) {
                self.forbidden += 1;
            }
            for argument in &called.args {
                self.visit_expr(argument);
            }
            return;
        }
        syn::visit::visit_expr_method_call(self, called);
    }

    fn visit_macro(&mut self, invoked: &'ast syn::Macro) {
        if !self.inside {
            return;
        }
        let writing = invoked
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "write" || segment.ident == "writeln");
        let mut tokens = invoked.tokens.clone().into_iter();
        let first = tokens.next();
        if writing
            && let Some(TokenTree::Ident(ident)) = &first
            && *ident == self.name.as_str()
        {
            self.forbidden += mentions(tokens.collect(), &self.name);
            return;
        }
        if let Some(first) = first {
            let mut rest: TokenStream = first.into();
            rest.extend(tokens);
            self.forbidden += mentions(rest, &self.name);
        }
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if self.inside && path.path.is_ident(&self.name) {
            self.forbidden += 1;
        }
        syn::visit::visit_expr_path(self, path);
    }
}

impl Watcher {
    /// Walks one function only when it holds the flagged site.
    fn enter(&mut self, span: proc_macro2::Span, walk: impl FnOnce(&mut Self)) {
        let range = ranged(span);
        if (range.start..range.end).contains(&self.site.start) {
            self.inside = true;
            walk(self);
            self.inside = false;
        }
    }
}

/// The count of `name` mentions in a token stream, groups included.
fn mentions(stream: TokenStream, name: &str) -> u32 {
    let mut found = 0;
    for token in stream {
        match token {
            TokenTree::Group(group) => found += mentions(group.stream(), name),
            TokenTree::Ident(ident) if ident == name => found += 1,
            _ => {}
        }
    }
    found
}

/// Whether the initializer is a `.lock()` call.
fn locking(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::MethodCall(called) if called.method == "lock")
}

/// The schema range of one span.
fn ranged(span: proc_macro2::Span) -> Range {
    let bytes = span.byte_range();
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}
