// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Observable output: what a function writes, to which stream, and under
//! which conditions.
//!
//! The effect list answers "something is printed" and cannot answer more:
//! it is sorted and deduplicated, so it carries neither payload, nor
//! order, nor the branch a write sits under. This module holds the
//! parallel channel that does — one record per write site, in body
//! order, never deduplicated, each carrying what the analysis can prove
//! and nothing beyond it.
//!
//! Honesty rules encoded in the types themselves: a payload whose text
//! is not fully proven cannot be rendered as a string
//! ([`Payload::fully_known`] refuses), a stream that cannot be
//! identified is recorded as such rather than omitted, and the version
//! of the recognizer list is stamped into every claim so absence reads
//! as "the recognizer found none", never as "there are none".

use std::fmt;

/// Version of the recognized write-sink list. Stamped into claims so a
/// negative result states which recognizer produced it.
pub const OUTPUT_SINK_SET: u32 = 1;

/// Stream a write goes to, as far as the analysis can prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stream {
    /// The process standard output stream.
    Stdout,
    /// The process standard error stream.
    Stderr,
    /// A writer the caller supplies — a generic parameter or trait
    /// object bound to the write trait, whose destination is decided
    /// outside this function.
    CallerSupplied,
    /// A write whose destination the analysis cannot identify.
    Unknown
}

impl fmt::Display for Stream {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::CallerSupplied => "caller-supplied",
            Self::Unknown => "unknown"
        };
        out.write_str(name)
    }
}

/// Whether the write appends a line terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Newline {
    /// A terminator is appended by the writing construct.
    Appended,
    /// No terminator is appended.
    None
}

/// Where a hole's value comes from in the format string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoleSource {
    /// An implicit positional argument, in order of appearance.
    Positional(u32),
    /// An explicit argument index.
    Indexed(u32),
    /// A named argument or captured identifier.
    Named(String)
}

/// One unproven position inside a format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// Which argument fills this position.
    pub source: HoleSource,
    /// Format specification as written, without the braces.
    pub spec:   String,
    /// Argument expression as written, kept as provenance only — it is
    /// never evaluated, folded, or propagated.
    pub expr:   Option<String>
}

/// One piece of a format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Text proven to appear verbatim.
    Text(String),
    /// A position whose value this analysis does not determine.
    Hole(Hole)
}

/// Why a payload could not be recovered as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueReason {
    /// The written value is a runtime expression, not a literal.
    RuntimeValue,
    /// The format string itself is not a literal.
    NonLiteralFormat,
    /// The construct's arguments did not parse as expressions.
    Unparsed,
    /// A debugging macro whose rendering embeds source position data
    /// this tier does not compute.
    DebugMacro
}

/// What a write puts on the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// Nothing is written — a flush or an empty write.
    Empty,
    /// Text proven in full.
    Literal(String),
    /// Proven text interleaved with holes.
    Template(Vec<Segment>),
    /// Nothing about the text is proven, with the reason.
    Opaque(OpaqueReason)
}

impl Payload {
    /// The written text when every part of it is proven, otherwise
    /// `None`.
    ///
    /// A template with a single hole is not fully known however much
    /// proven text surrounds it — the same rule the standard library's
    /// formatting arguments apply to their own string view.
    #[must_use]
    pub fn fully_known(&self) -> Option<&str> {
        match self {
            Self::Empty => Some(""),
            Self::Literal(text) => Some(text),
            Self::Template(_) | Self::Opaque(_) => None
        }
    }
}

/// One construct standing between a write and unconditional execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// The then-branch of a conditional.
    If,
    /// The else-branch of a conditional.
    Else,
    /// One arm of a match.
    MatchArm {
        /// Whether the arm's pattern accepts every remaining value.
        irrefutable: bool,
        /// Whether the arm carries an additional condition.
        guarded:     bool
    },
    /// A loop whose body may run zero times.
    For,
    /// A conditional loop whose body may run zero times.
    While,
    /// A closure body, which runs only where the closure is called.
    Closure,
    /// An item nested inside a body, which runs only when called.
    NestedItem,
    /// A configuration gate this tier does not resolve.
    CfgGated
}

/// Everything standing between one write site and certainty.
///
/// The two questions are distinct and are answered separately: `dead` is
/// a proof that the site never runs, `prior_exits` counts what stands
/// between the start of the function and the site without proving
/// either way. Neither being set is the third state — the site is
/// proven to be reached.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteGuard {
    /// Enclosing constructs, outermost first.
    pub guards:      Vec<Guard>,
    /// Statements before this site that may not hand control to it: an
    /// exit that may leave the function first, a divergence proven on
    /// some paths but not all, or a construct whose completion this tier
    /// does not resolve. Provenance for the reading, never the reading
    /// itself — one such statement weakens the site exactly as much as
    /// ten do.
    pub prior_exits: u32,
    /// Whether the site follows a proven divergence and can never run.
    pub dead:        bool
}

impl SiteGuard {
    /// A site under nothing at all.
    #[must_use]
    pub fn unconditional() -> Self {
        Self::default()
    }

    /// How certainly this site runs when its own function runs.
    #[must_use]
    pub const fn local_grade(&self) -> Grade {
        if self.dead {
            return Grade::Dead;
        }
        if self.guards.is_empty() && self.prior_exits == 0 {
            return Grade::Certain;
        }
        Grade::Possible
    }
}

/// How certainly something happens. The order is load-bearing: composing
/// two guarantees takes the weaker of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    /// Provably never runs.
    Dead,
    /// Not reachable from the entry point under analysis.
    Unreached,
    /// May run; something stands between it and certainty.
    Possible,
    /// Runs on every path.
    Certain
}

impl fmt::Display for Grade {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Dead => "dead",
            Self::Unreached => "unreached",
            Self::Possible => "possible",
            Self::Certain => "certain"
        };
        out.write_str(name)
    }
}

/// How many times a site runs per execution of its function.
///
/// Only the two readings the analysis can produce exist: a site outside
/// every repeating construct runs once, a site inside one runs a number
/// of times no syntax bounds. A counted bound would need the iteration
/// count of the enclosing construct, which this tier does not compute,
/// and a variant nothing constructs is a promise the engine does not
/// keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Multiplicity {
    /// Exactly once.
    Once,
    /// A loop or cycle bounds it with no static limit.
    Unbounded
}

/// One observed write to an output stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputWrite {
    /// Where the bytes go.
    pub stream:       Stream,
    /// What is written.
    pub payload:      Payload,
    /// Whether a line terminator is appended.
    pub newline:      Newline,
    /// Byte range of the writing construct.
    pub site:         rustmanifest_schema::Range,
    /// What stands between the site and unconditional execution.
    pub guard:        SiteGuard,
    /// How many times it runs per call.
    pub multiplicity: Multiplicity
}

/// Reads what a print-family macro invocation writes.
///
/// The first argument must be a string literal for any text to be
/// proven: a literal without placeholders is the payload verbatim, a
/// literal with placeholders becomes proven text interleaved with holes,
/// and anything else is opaque with the reason recorded. Argument
/// expressions are kept as provenance only — none of them is evaluated,
/// folded, or propagated, so a payload never claims a value the analysis
/// cannot see in the source.
///
/// A debugging macro is always opaque: its rendering embeds the source
/// position, which this tier does not compute.
pub(super) fn macro_payload(invocation: &syn::Macro, debug_macro: bool) -> Payload {
    if debug_macro {
        return Payload::Opaque(OpaqueReason::DebugMacro);
    }
    let Ok(arguments) = invocation.parse_body_with(
        syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
    ) else {
        return Payload::Opaque(OpaqueReason::Unparsed);
    };
    let mut arguments = arguments.into_iter();
    let Some(first) = arguments.next() else {
        return Payload::Empty;
    };
    let syn::Expr::Lit(literal) = &first else {
        return Payload::Opaque(OpaqueReason::NonLiteralFormat);
    };
    let text = match &literal.lit {
        syn::Lit::Str(value) => value.value(),
        syn::Lit::ByteStr(value) => String::from_utf8_lossy(&value.value()).into_owned(),
        _ => return Payload::Opaque(OpaqueReason::NonLiteralFormat)
    };
    let rest: Vec<String> = arguments
        .map(|argument| quote::quote!(#argument).to_string())
        .collect();
    template(&text, &rest)
}

/// Splits a format string into proven text and holes.
///
/// Escaped braces contribute their single character to the proven text;
/// a placeholder contributes a hole naming its argument. A placeholder
/// that never closes leaves the rest of the string unproven rather than
/// guessing where it ended.
fn template(text: &str, arguments: &[String]) -> Payload {
    let mut segments: Vec<Segment> = Vec::new();
    let mut proven = String::new();
    let mut positional = 0u32;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '{' if characters.peek() == Some(&'{') => {
                characters.next();
                proven.push('{');
            }
            '}' if characters.peek() == Some(&'}') => {
                characters.next();
                proven.push('}');
            }
            '{' => {
                let mut body = String::new();
                let mut closed = false;
                for inner in characters.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    body.push(inner);
                }
                if !closed {
                    return Payload::Opaque(OpaqueReason::Unparsed);
                }
                if !proven.is_empty() {
                    segments.push(Segment::Text(std::mem::take(&mut proven)));
                }
                segments.push(Segment::Hole(hole(&body, arguments, &mut positional)));
            }
            _ => proven.push(character)
        }
    }
    if segments.is_empty() {
        return Payload::Literal(proven);
    }
    if !proven.is_empty() {
        segments.push(Segment::Text(proven));
    }
    Payload::Template(segments)
}

/// Builds one hole from a placeholder body, consuming an implicit
/// positional argument when the placeholder names none.
fn hole(body: &str, arguments: &[String], positional: &mut u32) -> Hole {
    let (name, spec) = body.split_once(':').map_or((body, ""), |(n, s)| (n, s));
    let source = if name.is_empty() {
        let index = *positional;
        *positional += 1;
        HoleSource::Positional(index)
    } else if let Ok(index) = name.parse::<u32>() {
        HoleSource::Indexed(index)
    } else {
        HoleSource::Named(name.to_owned())
    };
    let expr = match &source {
        HoleSource::Positional(index) | HoleSource::Indexed(index) => {
            arguments.get(*index as usize).cloned()
        }
        HoleSource::Named(name) => arguments
            .iter()
            .find_map(|argument| argument.strip_prefix(&format!("{name} = ")))
            .map(ToOwned::to_owned)
            .or_else(|| Some(name.clone()))
    };
    Hole {
        source,
        spec: spec.to_owned(),
        expr
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
    use super::{
        Grade, Guard, Hole, HoleSource, OpaqueReason, Payload, Segment, SiteGuard, Stream
    };

    #[test]
    fn a_payload_is_fully_known_only_when_every_part_is_proven() {
        assert_eq!(Payload::Empty.fully_known(), Some(""));
        assert_eq!(
            Payload::Literal("Hello, world!".to_owned()).fully_known(),
            Some("Hello, world!")
        );
        let template = Payload::Template(vec![
            Segment::Text("count: ".to_owned()),
            Segment::Hole(Hole {
                source: HoleSource::Positional(0),
                spec:   String::new(),
                expr:   Some("total".to_owned())
            }),
        ]);
        assert_eq!(template.fully_known(), None);
        assert_eq!(
            Payload::Opaque(OpaqueReason::RuntimeValue).fully_known(),
            None
        );
    }

    #[test]
    fn grades_order_from_dead_to_certain() {
        assert!(Grade::Dead < Grade::Unreached);
        assert!(Grade::Unreached < Grade::Possible);
        assert!(Grade::Possible < Grade::Certain);
        assert_eq!(Grade::Possible.min(Grade::Certain), Grade::Possible);
    }

    #[test]
    fn a_site_is_certain_only_without_guards_and_without_prior_exits() {
        assert_eq!(SiteGuard::unconditional().local_grade(), Grade::Certain);
        let guarded = SiteGuard {
            guards:      vec![Guard::If],
            prior_exits: 0,
            dead:        false
        };
        assert_eq!(guarded.local_grade(), Grade::Possible);
        let after_exit = SiteGuard {
            guards:      Vec::new(),
            prior_exits: 1,
            dead:        false
        };
        assert_eq!(after_exit.local_grade(), Grade::Possible);
        let unreachable = SiteGuard {
            guards:      Vec::new(),
            prior_exits: 0,
            dead:        true
        };
        assert_eq!(unreachable.local_grade(), Grade::Dead);
    }

    fn payload_of(source: &str) -> Payload {
        let invocation: syn::Macro = syn::parse_str(source).unwrap();
        let debug = invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "dbg");
        super::macro_payload(&invocation, debug)
    }

    #[test]
    fn a_literal_without_placeholders_is_proven_verbatim() {
        assert_eq!(
            payload_of(r#"println!("Hello, world!")"#),
            Payload::Literal("Hello, world!".to_owned())
        );
        assert_eq!(
            payload_of(r#"print!("{{literal}}")"#),
            Payload::Literal("{literal}".to_owned())
        );
    }

    #[test]
    fn placeholders_become_holes_that_keep_their_argument_as_provenance() {
        let Payload::Template(segments) = payload_of(r#"println!("count: {}", total)"#) else {
            panic!("a placeholder must yield a template");
        };
        assert_eq!(segments[0], Segment::Text("count: ".to_owned()));
        let Segment::Hole(hole) = &segments[1] else {
            panic!("second segment must be a hole");
        };
        assert_eq!(hole.source, HoleSource::Positional(0));
        assert_eq!(hole.expr.as_deref(), Some("total"));

        let Payload::Template(named) = payload_of(r#"println!("hi {name}")"#) else {
            panic!("a captured identifier must yield a template");
        };
        let Segment::Hole(hole) = &named[1] else {
            panic!("second segment must be a hole");
        };
        assert_eq!(hole.source, HoleSource::Named("name".to_owned()));

        let Payload::Template(spec) = payload_of(concat!("println!(\"{0:>", "8.3}\", value)"))
        else {
            panic!("an indexed placeholder must yield a template");
        };
        let Segment::Hole(hole) = &spec[0] else {
            panic!("first segment must be a hole");
        };
        assert_eq!(hole.source, HoleSource::Indexed(0));
        assert_eq!(hole.spec, ">8.3");
    }

    #[test]
    fn anything_unproven_says_why_instead_of_guessing() {
        assert_eq!(
            payload_of("println!(GREETING)"),
            Payload::Opaque(OpaqueReason::NonLiteralFormat)
        );
        assert_eq!(
            payload_of("dbg!(value)"),
            Payload::Opaque(OpaqueReason::DebugMacro)
        );
        assert_eq!(
            payload_of(r#"println!("{")"#),
            Payload::Opaque(OpaqueReason::Unparsed)
        );
        assert_eq!(payload_of("println!()"), Payload::Empty);
    }

    #[test]
    fn a_greeting_records_one_stdout_write_with_its_proven_text() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            "fn main() {\n    println!(\"Hello, world!\");\n}".to_owned()
        )];
        let facts = behavior(&sources);
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert_eq!(main.output_writes.len(), 1);
        let write = &main.output_writes[0];
        assert_eq!(write.stream, super::Stream::Stdout);
        assert_eq!(write.payload.fully_known(), Some("Hello, world!"));
        assert_eq!(write.newline, super::Newline::Appended);
        assert_eq!(write.guard.local_grade(), Grade::Certain);
        assert!(write.site.end > write.site.start);
        assert_eq!(main.effects, vec![crate::semantic::Effect::ConsoleIo]);
    }

    #[test]
    fn each_write_is_its_own_event_in_body_order() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            "fn main() {\n    println!(\"one\");\n    eprintln!(\"two\");\n    println!(\"one\");\n}"
                .to_owned()
        )];
        let facts = behavior(&sources);
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        let observed: Vec<(super::Stream, Option<&str>)> = main
            .output_writes
            .iter()
            .map(|write| (write.stream, write.payload.fully_known()))
            .collect();
        assert_eq!(
            observed,
            [
                (super::Stream::Stdout, Some("one")),
                (super::Stream::Stderr, Some("two")),
                (super::Stream::Stdout, Some("one"))
            ]
        );
    }

    fn writes_of(body: &str) -> Vec<super::OutputWrite> {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            format!("fn main() {{\n{body}\n}}")
        )];
        behavior(&sources)
            .into_iter()
            .find(|fact| fact.function == "main")
            .map(|fact| fact.output_writes)
            .unwrap_or_default()
    }

    #[test]
    fn a_write_under_a_branch_is_possible_not_certain() {
        let branch = writes_of("    if flag { println!(\"hi\"); }");
        assert_eq!(branch[0].guard.guards, [Guard::If]);
        assert_eq!(branch[0].guard.local_grade(), Grade::Possible);

        let straight = writes_of("    println!(\"hi\");");
        assert!(straight[0].guard.guards.is_empty());
        assert_eq!(straight[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_loop_body_repeats_and_only_a_loop_that_may_not_run_weakens_it() {
        let counted = writes_of("    for _ in 0..n { println!(\"hi\"); }");
        assert_eq!(counted[0].guard.guards, [Guard::For]);
        assert_eq!(counted[0].multiplicity, super::Multiplicity::Unbounded);
        assert_eq!(counted[0].guard.local_grade(), Grade::Possible);

        let forever = writes_of("    loop { println!(\"hi\"); }");
        assert!(forever[0].guard.guards.is_empty());
        assert_eq!(forever[0].multiplicity, super::Multiplicity::Unbounded);
        assert_eq!(forever[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_closure_or_nested_item_body_only_runs_where_it_is_called() {
        let closure = writes_of("    let shout = || println!(\"hi\");");
        assert_eq!(closure[0].guard.guards, [Guard::Closure]);

        let nested = writes_of("    fn inner() { println!(\"hi\"); }");
        assert_eq!(nested[0].guard.guards, [Guard::NestedItem]);
    }

    #[test]
    fn an_exit_before_a_write_weakens_it_and_divergence_kills_it() {
        let after_try = writes_of("    helper()?;\n    println!(\"hi\");");
        assert_eq!(after_try[0].guard.prior_exits, 1);
        assert_eq!(after_try[0].guard.local_grade(), Grade::Possible);

        let after_return = writes_of("    return;\n    println!(\"hi\");");
        assert!(after_return[0].guard.dead);
        assert_eq!(after_return[0].guard.local_grade(), Grade::Dead);

        let after_panic = writes_of("    panic!(\"stop\");\n    println!(\"hi\");");
        assert!(after_panic[0].guard.dead);
    }

    #[test]
    fn a_configuration_gate_is_never_resolved_only_recorded() {
        let gated = writes_of("    #[cfg(feature = \"loud\")]\n    println!(\"hi\");");
        assert_eq!(gated[0].guard.guards, [Guard::CfgGated]);
        assert_eq!(gated[0].guard.local_grade(), Grade::Possible);
    }

    #[test]
    fn the_guard_stack_unwinds_so_a_later_write_is_not_poisoned() {
        let writes = writes_of(
            "    if a { match b { Some(_) => println!(\"deep\"), None => {} } }\n    \
             println!(\"flat\");"
        );
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].guard.guards.len(), 2);
        assert!(writes[1].guard.guards.is_empty());
    }

    #[test]
    fn a_divergence_below_the_top_of_a_statement_is_seen() {
        let branch = writes_of("    if flag { std::process::exit(1); }\n    println!(\"hi\");");
        assert!(!branch[0].guard.dead);
        assert_eq!(branch[0].guard.prior_exits, 1);
        assert_eq!(branch[0].guard.local_grade(), Grade::Possible);

        let nested = writes_of("    { panic!(\"stop\"); }\n    println!(\"hi\");");
        assert_eq!(nested[0].guard.local_grade(), Grade::Dead);

        let argument = writes_of("    helper(std::process::exit(1));\n    println!(\"hi\");");
        assert_eq!(argument[0].guard.local_grade(), Grade::Dead);
    }

    #[test]
    fn a_divergence_on_every_branch_kills_what_follows() {
        let branches = writes_of(
            "    if flag { return; } else { std::process::exit(1); }\n    println!(\"x\");"
        );
        assert!(branches[0].guard.dead);
        assert_eq!(branches[0].guard.local_grade(), Grade::Dead);

        let arms = writes_of(
            "    match v { Some(_) => return, None => panic!(\"no\") }\n    println!(\"x\");"
        );
        assert!(arms[0].guard.dead);
    }

    #[test]
    fn a_configuration_gated_divergence_weakens_what_follows_and_never_kills_it() {
        let gated = writes_of("    #[cfg(feature = \"x\")]\n    todo!();\n    println!(\"hi\");");
        assert!(!gated[0].guard.dead);
        assert_eq!(gated[0].guard.prior_exits, 1);
        assert_eq!(gated[0].guard.local_grade(), Grade::Possible);
    }

    #[test]
    fn a_configuration_gate_is_recorded_on_every_statement_shape() {
        let block = writes_of("    #[cfg(feature = \"loud\")]\n    unsafe { println!(\"hi\"); }");
        assert_eq!(block[0].guard.guards, [Guard::CfgGated]);

        let repeated =
            writes_of("    #[cfg(feature = \"loud\")]\n    for _ in 0..n { println!(\"hi\"); }");
        assert_eq!(repeated[0].guard.guards, [Guard::CfgGated, Guard::For]);

        let item = writes_of("    #[cfg(test)]\n    fn inner() { println!(\"hi\"); }");
        assert_eq!(item[0].guard.guards, [Guard::CfgGated, Guard::NestedItem]);
    }

    #[test]
    fn an_exit_inside_a_nested_body_does_not_weaken_the_body_around_it() {
        let nested = writes_of("    fn inner() { return; }\n    println!(\"outer\");");
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].guard.prior_exits, 0);
        assert_eq!(nested[0].guard.local_grade(), Grade::Certain);

        let closure = writes_of("    let quit = || return;\n    println!(\"outer\");");
        assert_eq!(closure[0].guard.prior_exits, 0);
        assert_eq!(closure[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_bare_loop_repeats_without_weakening_the_write_inside_it() {
        let forever = writes_of("    loop { println!(\"hi\"); }");
        assert!(forever[0].guard.guards.is_empty());
        assert_eq!(forever[0].guard.local_grade(), Grade::Certain);
        assert_eq!(forever[0].multiplicity, super::Multiplicity::Unbounded);
    }

    #[test]
    fn a_loop_that_nothing_breaks_out_of_kills_what_follows_it() {
        let endless = writes_of("    loop { helper(); }\n    println!(\"hi\");");
        assert_eq!(endless[0].guard.local_grade(), Grade::Dead);

        let escaping = writes_of("    loop { if flag { break; } }\n    println!(\"hi\");");
        assert!(!escaping[0].guard.dead);
        assert_eq!(escaping[0].guard.local_grade(), Grade::Possible);
    }

    #[test]
    fn a_macro_this_tier_cannot_see_through_weakens_what_follows() {
        let unknown = writes_of("    unknown_macro!();\n    println!(\"hi\");");
        assert_eq!(unknown[0].guard.prior_exits, 1);
        assert_eq!(unknown[0].guard.local_grade(), Grade::Possible);

        let settled = writes_of("    print!(\"first\");\n    println!(\"hi\");");
        assert_eq!(settled[1].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_binding_arm_is_irrefutable_only_without_a_subpattern() {
        let subpattern =
            writes_of("    match v { y @ Some(1) => println!(\"a\"), _ => println!(\"b\") }");
        assert_eq!(
            subpattern[0].guard.guards,
            [Guard::MatchArm {
                irrefutable: false,
                guarded:     false
            }]
        );
        assert_eq!(
            subpattern[1].guard.guards,
            [Guard::MatchArm {
                irrefutable: true,
                guarded:     false
            }]
        );

        let conditional =
            writes_of("    match v { y if y > 1 => println!(\"a\"), _ => println!(\"b\") }");
        assert_eq!(
            conditional[0].guard.guards,
            [Guard::MatchArm {
                irrefutable: true,
                guarded:     true
            }]
        );

        let binding = writes_of("    match v { y => println!(\"a\") }");
        assert!(binding[0].guard.guards.is_empty());
        assert_eq!(binding[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn streams_name_the_channel_they_stand_for() {
        assert_eq!(Stream::Stdout.to_string(), "stdout");
        assert_eq!(Stream::Stderr.to_string(), "stderr");
        assert_eq!(Stream::CallerSupplied.to_string(), "caller-supplied");
        assert_eq!(Stream::Unknown.to_string(), "unknown");
    }
}
