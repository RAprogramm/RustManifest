// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a compiler settles about the standard macros a fixture writes.
//!
//! Every reading of a macro answers from the item the invocation
//! reaches, because a program's own macro spelled `println` is not the
//! library's. A fixture of a few lines writes no such namesake, so the
//! written name settles to the library item of that name — and this
//! states it once for the whole file instead of a row per invocation.
//!
//! Stated here rather than in the readers: the readers must keep asking
//! the compiler, and a fixture must keep being able to answer for one.

use rustmanifest_schema::{
    Location, Range, Resolution, ResolvedCall, ResolvedMode, ResolvedParam, ResolvedPath,
    ResolvedSignature, ResolvedType
};

use crate::source::Source;

/// The library items the standard macros of a fixture reach, by the name
/// each one is written with.
const STANDARD_MACROS: [(&str, &str); 20] = [
    ("assert", "core::macros::builtin::assert"),
    ("assert_eq", "core::macros::assert_eq"),
    ("assert_ne", "core::macros::assert_ne"),
    ("dbg", "std::macros::dbg"),
    ("debug_assert", "core::macros::debug_assert"),
    ("debug_assert_eq", "core::macros::debug_assert_eq"),
    ("debug_assert_ne", "core::macros::debug_assert_ne"),
    ("eprint", "std::macros::eprint"),
    ("eprintln", "std::macros::eprintln"),
    ("format", "alloc::macros::format"),
    ("format_args", "std::macros::format_args"),
    ("panic", "core::macros::panic"),
    ("print", "std::macros::print"),
    ("println", "std::macros::println"),
    ("todo", "core::macros::todo"),
    ("unimplemented", "core::macros::unimplemented"),
    ("unreachable", "core::macros::unreachable"),
    ("vec", "alloc::macros::vec"),
    ("write", "core::macros::write"),
    ("writeln", "core::macros::writeln")
];

/// The macros whose invocation ends the path it stands on, so the
/// compiler types that position as one no value comes back from.
const LEAVING_MACROS: [&str; 4] = [
    "core::macros::panic",
    "core::macros::todo",
    "core::macros::unimplemented",
    "core::macros::unreachable"
];

/// The rendering the compiler gives a position no value comes back from.
const NEVER: &str = "!";

/// The library containers a fixture allocates, with the crate that
/// declares each — the crate is what makes a call into it an allocation.
const ALLOCATED: [(&str, &str); 5] = [
    ("Box", "alloc::boxed::Box"),
    ("String", "alloc::string::String"),
    ("Vec", "alloc::vec::Vec"),
    ("VecDeque", "alloc::collections::VecDeque"),
    ("Rc", "alloc::rc::Rc")
];

/// The constructors a fixture writes such a container with.
const CONSTRUCTORS: [&str; 3] = ["new", "with_capacity", "from"];

/// The outcome variants a fixture writes, each with the item it
/// reaches: which one a written name means is the compiler's answer,
/// and a fixture writes no namesake of its own.
const OUTCOME_VARIANTS: [(&str, &str); 4] = [
    ("Ok", "core::result::Ok"),
    ("Err", "core::result::Err"),
    ("Some", "core::option::Some"),
    ("None", "core::option::None")
];

/// The bodies that end the process, by the path a fixture writes them
/// with.
const ENDING: [(&str, &str); 4] = [
    ("std::process::exit", "std::process::exit"),
    ("process::exit", "std::process::exit"),
    ("std::process::abort", "std::process::abort"),
    ("process::abort", "std::process::abort")
];

/// The library types a fixture names outright, with the item that
/// declares each: a reading asking where a type lives gets the answer
/// only from a path, and a fixture writes the bare name.
const DECLARED_TYPES: [(&str, &str); 12] = [
    ("File", "std::fs::File"),
    ("TcpStream", "std::net::tcp::TcpStream"),
    ("TcpListener", "std::net::tcp::TcpListener"),
    ("UdpSocket", "std::net::udp::UdpSocket"),
    ("Command", "std::process::Command"),
    ("Stdout", "std::io::stdio::Stdout"),
    ("StdoutLock", "std::io::stdio::StdoutLock"),
    ("Stderr", "std::io::stdio::Stderr"),
    ("StderrLock", "std::io::stdio::StderrLock"),
    ("Stdin", "std::io::stdio::Stdin"),
    ("StdinLock", "std::io::stdio::StdinLock"),
    ("String", "alloc::string::String")
];

/// The process's own handle constructors a fixture writes, with the
/// body each reaches and the type it hands back.
const HANDLES: [(&str, &str, &str); 3] = [
    ("std::io::stdout", "std::io::stdio::stdout", "std::io::stdio::Stdout"),
    ("std::io::stderr", "std::io::stdio::stderr", "std::io::stdio::Stderr"),
    ("std::io::stdin", "std::io::stdio::stdin", "std::io::stdio::Stdin")
];

/// The numeric methods a fixture writes, each reaching the numeric
/// module of the language's own library.
const NUMERIC: [&str; 20] = [
    "checked_add",
    "checked_div",
    "checked_mul",
    "checked_neg",
    "checked_rem",
    "checked_sub",
    "overflowing_add",
    "overflowing_mul",
    "overflowing_sub",
    "saturating_add",
    "saturating_mul",
    "saturating_sub",
    "wrapping_add",
    "wrapping_div",
    "wrapping_mul",
    "wrapping_neg",
    "wrapping_rem",
    "wrapping_sub",
    "abs_diff",
    "pow"
];

/// The outcome methods a fixture writes, each with the body it reaches:
/// which of the two outcome types a call lands in is the compiler's
/// answer, and a fixture of a few lines writes the one its receiver has.
const OUTCOME_METHODS: [(&str, &str); 10] = [
    ("map_err", "core::result::Result::map_err"),
    ("unwrap_or_default", "core::result::Result::unwrap_or_default"),
    ("unwrap_or", "core::option::Option::unwrap_or"),
    ("unwrap", "core::option::Option::unwrap"),
    ("expect", "core::option::Option::expect"),
    ("ok_or", "core::option::Option::ok_or"),
    ("ok", "core::result::Result::ok"),
    ("and_then", "core::option::Option::and_then"),
    ("map", "core::option::Option::map"),
    ("cloned", "core::option::Option::cloned")
];

/// The library methods whose declared return is spelled out of the
/// function's own type parameters, which makes the type the caller's to
/// decide — the mark of a conversion.
const CALLER_DECIDED: [&str; 5] = ["parse", "into", "collect", "try_into", "to_owned"];

/// The methods that single out one element by a written position: the
/// compiler types their answer as an option, whether or not anything
/// stands under that key.
const POSITIONAL: [&str; 2] = ["nth", "get"];

/// The rendering the compiler gives such an answer.
const OPTIONAL: &str = "Option";

/// The container methods a fixture writes, with how each takes the
/// value it is written on: the compiler reads this off the receiver's
/// declared mode, and a fixture states it here.
const CONTAINER_METHODS: [(&str, ResolvedMode); 12] = [
    ("push", ResolvedMode::BorrowedMut),
    ("push_str", ResolvedMode::BorrowedMut),
    ("insert", ResolvedMode::BorrowedMut),
    ("extend", ResolvedMode::BorrowedMut),
    ("clear", ResolvedMode::BorrowedMut),
    ("pop", ResolvedMode::BorrowedMut),
    ("truncate", ResolvedMode::BorrowedMut),
    ("len", ResolvedMode::Borrowed),
    ("is_empty", ResolvedMode::Borrowed),
    ("iter", ResolvedMode::Borrowed),
    ("contains", ResolvedMode::Borrowed),
    ("capacity", ResolvedMode::Borrowed)
];

/// The methods that hand back a copy of what they are written on, which
/// the compiler states as a shared borrow answering with the receiver's
/// own type.
const COPYING: [&str; 3] = ["clone", "to_owned", "to_vec"];

/// The rendering a fixture states for a copy's receiver and answer
/// alike: the two are one type, which is the whole of what the reading
/// asks, and naming it would state more than the fixture knows.
const SAME_TYPE: &str = "Self";

/// Adds to `resolution` what a compiler settles about the standard
/// macros `sources` write.
pub fn settle_standard_macros(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, names) in STANDARD_MACROS {
            for at in written_at(text, written) {
                resolution.paths.push(ResolvedPath {
                    at:        Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    names:     names.to_owned(),
                    kind:      "macro".to_owned(),
                    declaring: names.split("::").next().unwrap_or(names).to_owned()
                });
                if LEAVING_MACROS.contains(&names)
                    && let Some(whole) = invocation_at(text, at)
                {
                    resolution.types.push(ResolvedType {
                        at:       Location {
                            file:  source.path().to_path_buf(),
                            range: whole
                        },
                        rendered: NEVER.to_owned(),
                        declared: None
                    });
                }
            }
        }
    }
    settle_allocations(resolution, sources);
    settle_copies(resolution, sources);
    settle_variants(resolution, sources);
    settle_endings(resolution, sources);
    settle_handles(resolution, sources);
    settle_numeric(resolution, sources);
    settle_casts(resolution, sources);
    settle_signatures(resolution, sources);
    settle_bindings(resolution, sources);
    settle_guards(resolution, sources);
    settle_container_methods(resolution, sources);
    settle_positional(resolution, sources);
    settle_conversions(resolution, sources);
    settle_outcome_methods(resolution, sources);
    settle_matched(resolution, sources);
    resolution.settle();
}

/// Records the constructor calls a fixture writes into the allocating
/// crate, so a reading asking which crate a call reaches is answered.
fn settle_allocations(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, declared) in ALLOCATED {
            for constructor in CONSTRUCTORS {
                let spelled = format!("{written}::{constructor}");
                for at in call_at(text, &spelled) {
                    resolution.paths.push(ResolvedPath {
                        at:        Location {
                            file:  source.path().to_path_buf(),
                            range: Range {
                                start: at.start,
                                end:   at.start
                                    + u32::try_from(spelled.len()).unwrap_or(u32::MAX)
                            }
                        },
                        names:     format!("{declared}::{constructor}"),
                        kind:      "function".to_owned(),
                        declaring: "alloc".to_owned()
                    });
                    resolution.calls.push(ResolvedCall {
                        at:             Location {
                            file:  source.path().to_path_buf(),
                            range: at
                        },
                        callee:         constructor.to_owned(),
                        returns:        written.to_owned(),
                        receiver:       None,
                        receiver_path:  None,
                        caller_decides: false,
                        reaches:        Some(format!("{declared}::{constructor}")),
                        takes:          None
                    });
                }
            }
        }
    }
}

/// Records the outcome variants a fixture writes, as the items they
/// reach.
fn settle_variants(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, names) in OUTCOME_VARIANTS {
            for at in name_at(text, written) {
                resolution.paths.push(ResolvedPath {
                    at:        Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    names:     names.to_owned(),
                    kind:      "variant".to_owned(),
                    declaring: "core".to_owned()
                });
            }
        }
    }
}

/// Records the process-ending calls a fixture writes, as the bodies
/// they reach.
fn settle_endings(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, names) in ENDING {
            for at in call_at(text, written) {
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    callee:         names.rsplit("::").next().unwrap_or(names).to_owned(),
                    returns:        NEVER.to_owned(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(names.to_owned()),
                    takes:          None
                });
            }
        }
    }
}

/// Records the type of every value a `match` decides over, where its
/// arms name the variants of one outcome type.
///
/// Which type a choice is closed over is the compiler's answer, and a
/// fixture writing `Ok`/`Err` arms is deciding over a fallible value.
fn settle_matched(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let Some(parsed) = source.tree() else {
            continue;
        };
        let mut matched = Matched {
            resolution,
            source
        };
        syn::visit::Visit::visit_file(&mut matched, &parsed);
    }
}

/// A walk recording the type each written choice decides over.
struct Matched<'a> {
    /// What is being settled.
    resolution: &'a mut Resolution,
    /// File the choice is written in.
    source:     &'a Source
}

impl<'ast> syn::visit::Visit<'ast> for Matched<'_> {
    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        use syn::spanned::Spanned as _;

        let written: Vec<String> = matched
            .arms
            .iter()
            .filter_map(|arm| match &arm.pat {
                syn::Pat::TupleStruct(held) => {
                    Some(held.path.segments.last()?.ident.to_string())
                }
                syn::Pat::Path(held) => Some(held.path.segments.last()?.ident.to_string()),
                syn::Pat::Ident(held) => Some(held.ident.to_string()),
                _ => None
            })
            .collect();
        let decided = if written.iter().any(|name| name == "Ok") {
            Some("Result")
        } else if written.iter().any(|name| name == "Some") {
            Some("Option")
        } else {
            None
        };
        if let Some(decided) = decided {
            let span = matched.expr.span().byte_range();
            self.resolution.types.push(ResolvedType {
                at:       Location {
                    file:  self.source.path().to_path_buf(),
                    range: Range {
                        start: u32::try_from(span.start).unwrap_or(u32::MAX),
                        end:   u32::try_from(span.end).unwrap_or(u32::MAX)
                    }
                },
                rendered: decided.to_owned(),
                declared: None
            });
        }
        syn::visit::visit_expr_match(self, matched);
    }
}

/// Records which body each outcome method a fixture writes reaches.
fn settle_outcome_methods(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (method, reaches) in OUTCOME_METHODS {
            let spelled = format!(".{method}(");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len() - 1;
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        String::new(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(reaches.to_owned()),
                    takes:          None
                });
            }
        }
    }
}

/// Records the calls whose answer the caller decides, which is what
/// makes a call a conversion rather than an access.
fn settle_conversions(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for method in CALLER_DECIDED {
            let spelled = format!(".{method}(");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len() - 1;
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        String::new(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: true,
                    reaches:        None,
                    takes:          None
                });
            }
        }
    }
}

/// Records the optional answer a positional call hands back.
fn settle_positional(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for method in POSITIONAL {
            let spelled = format!(".{method}(");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len() - 1;
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.types.push(ResolvedType {
                    at:       Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    rendered: OPTIONAL.to_owned(),
                    declared: None
                });
            }
        }
    }
}

/// Records how each container method a fixture writes takes the value
/// it is written on.
fn settle_container_methods(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (method, mode) in CONTAINER_METHODS {
            let spelled = format!(".{method}(");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len() - 1;
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        String::new(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        None,
                    takes:          Some(mode)
                });
            }
        }
    }
}

/// The guard each process stream's locking call hands out.
const GUARDS: [(&str, &str); 3] = [
    ("std::io::stdio::Stdout", "std::io::stdio::StdoutLock"),
    ("std::io::stdio::Stderr", "std::io::stdio::StderrLock"),
    ("std::io::stdio::Stdin", "std::io::stdio::StdinLock")
];

/// Records the guard a locking call hands out, and the name a `let`
/// binds it to.
///
/// Reads what this settler already stated: a handle is known by the
/// constructor that built it or by the name it was bound to, and the
/// guard follows from the handle.
fn settle_guards(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        let file = source.path().to_path_buf();
        let mut settled: Vec<(Range, String)> = Vec::new();
        let mut from = 0usize;
        while let Some(at) = text.get(from..).and_then(|rest| rest.find(".lock()")) {
            let start = from + at;
            from = start + ".lock()".len();
            let Some(whole) = written_call(text, start + ".lock".len()) else {
                continue;
            };
            let held = text.get(whole.start as usize..start).unwrap_or_default();
            let Some(handle) = handle_of(
                resolution,
                &file,
                u32::try_from(start).unwrap_or(u32::MAX)
            )
            .or_else(|| named_handle(resolution, &file, text, held.trim())) else {
                continue;
            };
            let Some((_, guard)) = GUARDS.iter().find(|(named, _)| *named == handle) else {
                continue;
            };
            settled.push((whole, (*guard).to_owned()));
            if let Some(bound) = bound_name(text, whole.start as usize) {
                settled.push((bound, (*guard).to_owned()));
            }
        }
        for (at, guard) in settled {
            resolution.types.push(ResolvedType {
                at:       Location {
                    file: file.clone(),
                    range: at
                },
                rendered: guard.rsplit("::").next().unwrap_or(&guard).to_owned(),
                declared: Some(guard)
            });
        }
    }
}

/// The handle a written name holds, as this settler stated it at the
/// binding: a name written in one statement and locked in the next
/// stands at two positions and is one binding.
fn named_handle(
    resolution: &Resolution,
    file: &std::path::Path,
    text: &str,
    written: &str
) -> Option<String> {
    if written.is_empty() || !written.chars().all(|letter| letter.is_alphanumeric() || letter == '_')
    {
        return None;
    }
    resolution
        .types
        .iter()
        .filter(|row| row.at.file == file && row.declared.is_some())
        .find(|row| {
            text.get(row.at.range.start as usize..row.at.range.end as usize) == Some(written)
        })
        .and_then(|row| row.declared.clone())
}

/// The handle a locking call is written on, as this settler already
/// stated it: the receiver is whatever ends where the call begins.
fn handle_of(resolution: &Resolution, file: &std::path::Path, ends: u32) -> Option<String> {
    resolution
        .types
        .iter()
        .find(|row| row.at.file == file && row.at.range.end == ends && row.declared.is_some())
        .and_then(|row| row.declared.clone())
}

/// Records the type every name a fixture binds with a written type
/// holds, and where an item declares that type.
fn settle_bindings(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let Some(parsed) = source.tree() else {
            continue;
        };
        let mut held = Vec::new();
        crate::ast::flattened(&parsed.items, &mut held);
        for item in held {
            let mut named = Named {
                resolution,
                source
            };
            syn::visit::Visit::visit_item(&mut named, item);
        }
    }
}

/// A walk recording every written binding of a fixture.
struct Named<'a> {
    /// What is being settled.
    resolution: &'a mut Resolution,
    /// File the binding is written in.
    source:     &'a Source
}

impl Named<'_> {
    /// Records one name as holding the type written beside it.
    fn record(&mut self, name: &syn::Ident, written: &syn::Type) {

        let bytes = name.span().byte_range();
        let rendered = written_rendering(written);
        let bare = rendered.trim_start_matches('&').trim_start_matches("mut");
        let declared = DECLARED_TYPES
            .iter()
            .find(|(spelled, _)| *spelled == bare)
            .map(|(_, path)| (*path).to_owned());
        self.resolution.types.push(ResolvedType {
            at:       Location {
                file:  self.source.path().to_path_buf(),
                range: Range {
                    start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
                }
            },
            rendered: rendered.clone(),
            declared
        });
    }
}

impl<'ast> syn::visit::Visit<'ast> for Named<'_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Type(typed) = &local.pat
            && let syn::Pat::Ident(name) = typed.pat.as_ref()
        {
            self.record(&name.ident, &typed.ty);
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_fn_arg(&mut self, taken: &'ast syn::FnArg) {
        if let syn::FnArg::Typed(held) = taken
            && let syn::Pat::Ident(name) = held.pat.as_ref()
        {
            self.record(&name.ident, &held.ty);
        }
        syn::visit::visit_fn_arg(self, taken);
    }
}

/// Records the signature of every function a fixture declares: what it
/// takes, how, and what it hands back.
///
/// The written signature is the settled one for a fixture of a few
/// lines — no alias stands between them — and every reading above asks
/// the compiler for it.
fn settle_signatures(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let Some(parsed) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&parsed.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(declared) => {
                    record_signature(resolution, source, &declared.sig, None);
                }
                syn::Item::Impl(block) => {
                    let owner = crate::semantic::expr::impl_type_name(block);
                    for held in &block.items {
                        if let syn::ImplItem::Fn(method) = held {
                            record_signature(resolution, source, &method.sig, owner.as_deref());
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Records one signature, the receiver named by the type it stands in.
fn record_signature(
    resolution: &mut Resolution,
    source: &Source,
    signature: &syn::Signature,
    owner: Option<&str>
) {

    let bytes = signature.ident.span().byte_range();
    let mut params = Vec::new();
    for taken in &signature.inputs {
        match taken {
            syn::FnArg::Receiver(held) => params.push(ResolvedParam {
                name:  "self".to_owned(),
                named: owner.unwrap_or("Self").to_owned(),
                mode:  match &held.kind {
                    syn::ReceiverKind::Reference(_, _, Some(_)) => ResolvedMode::BorrowedMut,
                    syn::ReceiverKind::Reference(_, _, None) => ResolvedMode::Borrowed,
                    _ => ResolvedMode::Owned
                }
            }),
            syn::FnArg::Typed(held) => {
                let syn::Pat::Ident(named) = held.pat.as_ref() else {
                    continue;
                };
                let written = written_rendering(&held.ty);
                params.push(ResolvedParam {
                    name:  named.ident.to_string(),
                    mode:  if written.starts_with("&mut") {
                        ResolvedMode::BorrowedMut
                    } else if written.starts_with('&') {
                        ResolvedMode::Borrowed
                    } else {
                        ResolvedMode::Owned
                    },
                    named: written
                });
            }
        }
    }
    let returns = match &signature.output {
        syn::ReturnType::Default => "()".to_owned(),
        syn::ReturnType::Type(_, written) => written_rendering(written)
    };
    resolution.signatures.push(ResolvedSignature {
        at: Location {
            file:  source.path().to_path_buf(),
            range: Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            }
        },
        params,
        returns
    });
}

/// A written type as one rendering, its lifetimes dropped, the way the
/// compiler renders it.
fn written_rendering(written: &syn::Type) -> String {
    crate::semantic::settling::written_rendering(written)
}

/// Records the type a written cast produces: the source writes it, and
/// the compiler settles the cast's own position to it.
fn settle_casts(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        let mut from = 0usize;
        while let Some(at) = text.get(from..).and_then(|rest| rest.find(" as ")) {
            let start = from + at;
            let named = start + " as ".len();
            from = named;
            let end = text[named..]
                .find(|letter: char| !(letter.is_alphanumeric() || letter == '_'))
                .map_or(text.len(), |held| named + held);
            let written = &text[named..end];
            if written.is_empty() {
                continue;
            }
            let opens = text[..start]
                .char_indices()
                .rev()
                .take_while(|(_, letter)| {
                    letter.is_alphanumeric() || *letter == '_' || *letter == '.'
                })
                .last()
                .map_or(start, |(held, _)| held);
            resolution.types.push(ResolvedType {
                at:       Location {
                    file:  source.path().to_path_buf(),
                    range: Range {
                        start: u32::try_from(opens).unwrap_or(u32::MAX),
                        end:   u32::try_from(end).unwrap_or(u32::MAX)
                    }
                },
                rendered: written.to_owned(),
                declared: None
            });
        }
    }
}

/// Records the handle constructors a fixture writes, as the bodies they
/// reach and the types they hand back.
fn settle_handles(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, reaches, declared) in HANDLES {
            for at in call_at(text, written) {
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    callee:         reaches.rsplit("::").next().unwrap_or(reaches).to_owned(),
                    returns:        declared.rsplit("::").next().unwrap_or(declared).to_owned(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(reaches.to_owned()),
                    takes:          None
                });
                resolution.types.push(ResolvedType {
                    at:       Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    rendered: declared.rsplit("::").next().unwrap_or(declared).to_owned(),
                    declared: Some(declared.to_owned())
                });
                let whole_value = text
                    .get(at.end as usize..)
                    .is_some_and(|rest| rest.trim_start().starts_with(';'));
                if whole_value
                    && let Some(bound) = bound_name(text, at.start as usize)
                {
                    resolution.types.push(ResolvedType {
                        at:       Location {
                            file:  source.path().to_path_buf(),
                            range: bound
                        },
                        rendered: declared.rsplit("::").next().unwrap_or(declared).to_owned(),
                        declared: Some(declared.to_owned())
                    });
                }
            }
        }
    }
}

/// The name a `let` binds the value built at `built` to, where the
/// statement writes one.
fn bound_name(text: &str, built: usize) -> Option<Range> {
    let head = text.get(..built)?.trim_end();
    let head = head.strip_suffix('=')?.trim_end();
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, letter)| letter.is_alphanumeric() || *letter == '_')
        .last()
        .map(|(at, _)| at)?;
    let name = &head[start..];
    let before = head[..start].trim_end();
    if !before.ends_with("let") && !before.ends_with("mut") {
        return None;
    }
    (!name.is_empty()).then(|| Range {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end:   u32::try_from(start + name.len()).unwrap_or(u32::MAX)
    })
}

/// Records the numeric calls a fixture writes, as the bodies of the
/// numeric module they reach.
fn settle_numeric(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for method in NUMERIC {
            let spelled = format!(".{method}");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len();
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        String::new(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(format!("core::num::{method}")),
                    takes:          None
                });
            }
        }
    }
}

/// Every range a bare written name stands at, where nothing joins it to
/// a longer one.
fn name_at(text: &str, written: &str) -> Vec<Range> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = text.get(from..).and_then(|rest| rest.find(written)) {
        let start = from + at;
        let end = start + written.len();
        from = end;
        let joined_before = text[..start]
            .chars()
            .next_back()
            .is_some_and(|letter| letter.is_alphanumeric() || letter == '_');
        let joined_after = text[end..]
            .chars()
            .next()
            .is_some_and(|letter| letter.is_alphanumeric() || letter == '_');
        if !joined_before && !joined_after {
            found.push(Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(end).unwrap_or(u32::MAX)
            });
        }
    }
    found
}

/// Records the copying calls a fixture writes, as the compiler states
/// them: taken by shared borrow, answering with the receiver's own type.
fn settle_copies(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for method in COPYING {
            let spelled = format!(".{method}");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len();
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        SAME_TYPE.to_owned(),
                    receiver:       Some(SAME_TYPE.to_owned()),
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(format!("core::clone::Clone::{method}")),
                    takes:          Some(rustmanifest_schema::ResolvedMode::Borrowed)
                });
            }
        }
    }
}

/// The range the whole call written on a receiver stands at: from the
/// start of the receiver's own expression to the closing bracket.
fn written_call(text: &str, opens: usize) -> Option<Range> {
    let closes = closed_at(text, 0, opens)?.end as usize;
    let head = text.get(..opens)?;
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, letter)| {
            letter.is_alphanumeric()
                || *letter == '_'
                || *letter == '.'
                || *letter == ')'
                || *letter == '('
                || *letter == ':'
        })
        .last()
        .map_or(opens, |(at, _)| at);
    Some(Range {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end:   u32::try_from(closes).unwrap_or(u32::MAX)
    })
}

/// Every range a written call stands at, its argument list closed.
fn call_at(text: &str, spelled: &str) -> Vec<Range> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = text.get(from..).and_then(|rest| rest.find(spelled)) {
        let start = from + at;
        let opens = start + spelled.len();
        from = opens;
        let Some(range) = closed_at(text, start, opens) else {
            continue;
        };
        found.push(range);
    }
    found
}

/// The range a call starting at `start` stands at, where its argument
/// list opens at `opens` and closes further on.
fn closed_at(text: &str, start: usize, opens: usize) -> Option<Range> {
    if !text.get(opens..)?.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (at, letter) in text.get(opens..)?.char_indices() {
        if letter == '(' {
            depth += 1;
        } else if letter == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(Range {
                    start: u32::try_from(start).unwrap_or(u32::MAX),
                    end:   u32::try_from(opens + at + 1).unwrap_or(u32::MAX)
                });
            }
        }
    }
    None
}

/// What a compiler settles about the standard macros `sources` write,
/// and nothing else.
#[must_use]
pub fn standard_macros_over(sources: &[Source]) -> Resolution {
    let mut resolution = Resolution {
        resolver: "test".to_owned(),
        ..Resolution::default()
    };
    settle_standard_macros(&mut resolution, sources);
    resolution
}

/// Every range the name of a macro invocation stands at: the name
/// itself, followed by the bang that makes it one.
fn written_at(text: &str, written: &str) -> Vec<Range> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = text.get(from..).and_then(|rest| rest.find(written)) {
        let start = from + at;
        let end = start + written.len();
        from = end;
        if text[end..].starts_with('!')
            && !text[..start]
                .chars()
                .next_back()
                .is_some_and(|letter| letter.is_alphanumeric() || letter == '_')
        {
            found.push(Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(end).unwrap_or(u32::MAX)
            });
        }
    }
    found
}

/// The range the whole invocation written at `name` stands at, its
/// argument list closed.
fn invocation_at(text: &str, name: Range) -> Option<Range> {
    let after = name.end as usize + 1;
    let opening = text.get(after..)?.chars().next()?;
    let closing = match opening {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None
    };
    let mut depth = 0usize;
    for (at, letter) in text.get(after..)?.char_indices() {
        if letter == opening {
            depth += 1;
        } else if letter == closing {
            depth -= 1;
            if depth == 0 {
                return Some(Range {
                    start: name.start,
                    end:   u32::try_from(after + at + 1).unwrap_or(u32::MAX)
                });
            }
        }
    }
    None
}
