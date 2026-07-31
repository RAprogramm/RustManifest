// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `constructor-design` check: constructors only assign fields; processing,
//! validation, and I/O belong in methods (`STRUCTURE.md` §5).
//!
//! The heuristic is deliberately narrow. An inherent-impl method named `new`
//! is reported when it returns `Result<...>`, or when its body contains the
//! `?` operator, a `.unwrap()`/`.expect(..)` method call, or a call whose
//! path names `read_to_string`, `read`, `write`, `open`, or `create` inside
//! `std::fs`/`fs`. Any other logic — loops, parsing, arbitrary function
//! calls — is out of scope for this check.

use syn::visit::Visit;

use super::{Occurrence, flattened};

/// Filesystem functions that mark a constructor as doing I/O.
const FS_CALLS: [&str; 5] = ["read_to_string", "read", "write", "open", "create"];

/// Reports inherent-impl `new` constructors that do more than assign fields.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        let syn::Item::Impl(block) = item else {
            continue;
        };
        if block.trait_.is_some() {
            continue;
        }
        for member in &block.items {
            let syn::ImplItem::Fn(method) = member else {
                continue;
            };
            if method.sig.ident != "new" {
                continue;
            }
            if let Some(reason) = violation(method) {
                found.push(Occurrence::new(
                    &method.sig.ident.span(),
                    format!(
                        "constructor `new` {reason}; constructors only assign fields — move the \
                         logic into a method"
                    )
                ));
            }
        }
    }
    found
}

/// Returns why the constructor violates the check, when it does.
fn violation(method: &syn::ImplItemFn) -> Option<&'static str> {
    if returns_result(&method.sig.output) {
        return Some("is fallible (returns `Result`)");
    }
    let mut scan = LogicScan {
        verdict: None
    };
    scan.visit_block(&method.block);
    scan.verdict
}

/// True when the return type is a `Result<...>` path.
fn returns_result(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else {
        return false;
    };
    let syn::Type::Path(path) = ty.as_ref() else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result")
}

/// Expression visitor recording the first logic marker found in a body.
struct LogicScan {
    /// Human-readable description of the first marker encountered.
    verdict: Option<&'static str>
}

impl<'ast> Visit<'ast> for LogicScan {
    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.verdict.get_or_insert("propagates errors with `?`");
        syn::visit::visit_expr_try(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "unwrap" {
            self.verdict.get_or_insert("calls `.unwrap()`");
        } else if node.method == "expect" {
            self.verdict.get_or_insert("calls `.expect(..)`");
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if fs_call(&node.func) {
            self.verdict.get_or_insert("performs filesystem I/O");
        }
        syn::visit::visit_expr_call(self, node);
    }
}

/// True when `func` is a path call into `std::fs`/`fs` naming one of
/// [`FS_CALLS`].
fn fs_call(func: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = func else {
        return false;
    };
    let segments = &path.path.segments;
    let Some(last) = segments.last() else {
        return false;
    };
    let name = last.ident.to_string();
    FS_CALLS.contains(&name.as_str())
        && segments
            .iter()
            .rev()
            .skip(1)
            .any(|segment| segment.ident == "fs")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::occurrences;

    fn count(source: &str) -> usize {
        occurrences(&syn::parse_file(source).unwrap()).len()
    }

    #[test]
    fn fallible_new_is_reported() {
        let source = "struct C { x: u8 } impl C { fn new() -> Result<Self, ()> { Ok(Self { x: 0 \
                      }) } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn try_operator_in_new_is_reported() {
        let source =
            "struct C { x: u8 } impl C { fn new() -> Self { let x = parse()?; Self { x } } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn fs_call_in_new_is_reported() {
        let source = "struct C { data: String } impl C { fn new(p: &str) -> Self { let data = \
                      std::fs::read_to_string(p).unwrap_or_default(); Self { data } } }";
        assert_eq!(count(source), 1);
        let source = "struct C { data: Vec<u8> } impl C { fn new(p: &str) -> Self { let data = \
             fs::read(p).unwrap_or_default(); Self { data } } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn unwrap_and_expect_in_new_are_reported() {
        let source = "struct C { x: u8 } impl C { fn new() -> Self { Self { x: \
                      parse().unwrap() } } }";
        assert_eq!(count(source), 1);
        let source = "struct C { x: u8 } impl C { fn new() -> Self { Self { x: \
                      parse().expect(\"x\") } } }";
        assert_eq!(count(source), 1);
    }

    #[test]
    fn plain_assignment_new_passes() {
        let source = "struct C { x: u8 } impl C { fn new(x: u8) -> Self { Self { x } } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn logic_outside_new_passes() {
        let source = "struct C { p: String } impl C { fn data(&self) -> String { \
                      std::fs::read_to_string(&self.p).unwrap() } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn io_names_outside_fs_pass() {
        let source = "struct C { f: u8 } impl C { fn new() -> Self { let f = open(\"x\"); Self { \
                      f } } }";
        assert_eq!(count(source), 0);
    }

    #[test]
    fn trait_impl_new_passes() {
        let source = "struct C; trait Make { fn new() -> Self; } impl Make for C { fn new() -> \
                      Self { helper().unwrap() } }";
        assert_eq!(count(source), 0);
    }
}
