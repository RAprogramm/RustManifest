// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `use-after-move` check: a name mentioned after its value provably
//! left it.
//!
//! The reading follows each function body in written order and tracks
//! only what it can prove: a local whose type is provably not `Copy`
//! (the std owner tables, or a workspace type without `Copy`), moved by
//! a bare-name by-value pass into a call, by a known consuming method,
//! or by a bare-name `let`. Any later mention of the name — a path, a
//! reference, a receiver, or an inline template capture inside a macro
//! — is flagged where it stands: the compiler rejects it as E0382, and
//! this check says so before any compiler runs, in the language of the
//! recorded facts.
//!
//! The conservatism is deliberate: a rebinding or assignment revives
//! the name, and a move written under a branch, loop, or closure is
//! not recorded at all, because whether it runs is not provable from
//! the text. A silent miss is a smaller claim; a false alarm here
//! would be a lie about a program that compiles.

mod walk;

use rustmanifest_schema::{Range, TraceStep};

use super::{insight::Insight, workspace::Workspace};

/// One proven use of a moved-from name.
pub(super) struct MovedUse {
    /// Name as written.
    pub(super) name:      String,
    /// Provably non-`Copy` type of the value that left.
    pub(super) type_name: String,
    /// What the value moved into, as written.
    pub(super) into:      String,
    /// Byte range of the later mention.
    pub(super) range:     Range
}

/// Reports every provable use of a moved-from name, one insight per
/// mention.
pub(super) fn insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let types = workspace.types();
    let mut found = Vec::new();
    for (index, source) in workspace.sources().iter().enumerate() {
        let Ok(parsed) = syn::parse_file(source.text()) else {
            continue;
        };
        for held in walk::moved_uses(&parsed, types) {
            found.push(told(index, held));
        }
    }
    found
}

/// The insight of one mention: the claim and the steps carrying it.
fn told(source: usize, held: MovedUse) -> Insight {
    let MovedUse {
        name,
        type_name,
        into,
        range
    } = held;
    let detail = format!(
        "`{name}` is used after its value moved into `{into}`: `{type_name}` is not `Copy`, so \
         the by-value pass left `{name}` dead, and the compiler rejects this use as E0382"
    );
    let trace = vec![
        TraceStep {
            fact:   format!("`{name}` was handed to `{into}` by value"),
            reason: "a bare name written as an argument gives the value away".to_owned()
        },
        TraceStep {
            fact:   format!("`{type_name}` is not `Copy`"),
            reason: "the std owner tables and the workspace derives and impls prove it".to_owned()
        },
        TraceStep {
            fact:   format!("`{name}` is mentioned again after the move"),
            reason: "a moved-from name no longer holds a value".to_owned()
        },
    ];
    Insight::new(source, range, detail, trace)
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

    use super::insights;
    use crate::{semantic::workspace::Workspace, source::Source};

    fn found(text: &str) -> usize {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        insights(&Workspace::new(&sources)).len()
    }

    #[test]
    fn a_use_after_a_by_value_pass_is_flagged() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let s = String::from(\"x\"); eat(s); \
                 let n = s.len(); }"
            ),
            1
        );
    }

    #[test]
    fn an_inline_template_capture_after_the_move_is_flagged() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let s = String::from(\"x\"); eat(s); \
                 println!(\"{s}\"); }"
            ),
            1
        );
    }

    #[test]
    fn a_borrow_is_not_a_move() {
        assert_eq!(
            found(
                "fn read(value: &String) {}\nfn main() { let s = String::from(\"x\"); read(&s); \
                 println!(\"{s}\"); }"
            ),
            0
        );
    }

    #[test]
    fn a_rebinding_revives_the_name() {
        assert_eq!(
            found(
                "fn pass(value: String) -> String { value }\nfn main() { let s = \
                 String::from(\"x\"); let s = pass(s); println!(\"{s}\"); }"
            ),
            0
        );
    }

    #[test]
    fn an_assignment_revives_the_name() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let mut s = String::from(\"x\"); eat(s); \
                 s = String::from(\"y\"); println!(\"{s}\"); }"
            ),
            0
        );
    }

    #[test]
    fn a_copy_value_is_never_flagged() {
        assert_eq!(
            found("fn eat(value: u32) {}\nfn main() { let n = 7u32; eat(n); println!(\"{n}\"); }"),
            0
        );
    }

    #[test]
    fn a_move_under_a_branch_is_not_recorded() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let s = String::from(\"x\"); if \
                 s.is_empty() { eat(s); } else { drop(s); } }"
            ),
            0
        );
    }

    #[test]
    fn a_consuming_method_moves_the_receiver() {
        assert_eq!(
            found(
                "fn main() { let s = String::from(\"x\"); let held = s.into_bytes(); \
                 println!(\"{s}\"); }"
            ),
            1
        );
    }

    #[test]
    fn a_bare_let_moves_the_name() {
        assert_eq!(
            found("fn main() { let s = String::from(\"x\"); let kept = s; println!(\"{s}\"); }"),
            1
        );
    }

    #[test]
    fn a_workspace_non_copy_type_is_tracked_through_a_constructor() {
        assert_eq!(
            found(
                "struct Token { text: String }\nimpl Token { fn new() -> Self { Token { text: \
                 String::new() } } }\nfn eat(token: Token) {}\nfn main() { let t = Token::new(); \
                 eat(t); let again = &t; }"
            ),
            1
        );
    }

    #[test]
    fn an_owned_parameter_is_tracked_from_the_signature() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn keep(value: String) { eat(value); \
                 println!(\"{value}\"); }"
            ),
            1
        );
    }

    #[test]
    fn a_type_ascribed_rebinding_revives_the_name() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let s = String::from(\"x\"); eat(s); let \
                 s: String = String::from(\"y\"); println!(\"{s}\"); }"
            ),
            0
        );
    }

    #[test]
    fn a_closure_parameter_shadows_the_moved_name() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let s = String::from(\"x\"); eat(s); let \
                 tell = |s: String| s.len(); tell(String::new()); }"
            ),
            0
        );
    }

    #[test]
    fn configuration_gated_statements_prove_nothing() {
        assert_eq!(
            found(
                "fn eat(value: String) {}\nfn main() { let s = String::from(\"x\");\n    \
                 #[cfg(feature = \"a\")]\n    eat(s);\n    #[cfg(not(feature = \"a\"))]\n    \
                 println!(\"{s}\");\n}"
            ),
            0
        );
    }

    #[test]
    fn a_foreign_import_shadowing_a_workspace_type_name_is_not_judged() {
        let sources = [
            Source::new(
                PathBuf::from("src/layout.rs"),
                "pub struct Layout { pub text: String }".to_owned()
            ),
            Source::new(
                PathBuf::from("src/view.rs"),
                "use iced::Layout;\nfn eat(layout: Layout) {}\nfn main() { let l = \
                 Layout::new();\n    eat(l);\n    let again = &l;\n}"
                    .to_owned()
            )
        ];
        assert_eq!(insights(&Workspace::new(&sources)).len(), 0);
    }
}
