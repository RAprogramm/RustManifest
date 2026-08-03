// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Body analysis: the syntax visitor that accumulates behavioral facts —
//! reads, writes, constructions, effects, clones, and allocations — from
//! one function body against its signature-derived parameter types.

use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;

use super::{
    Returned, Scoped,
    signature::{element_type, type_last_segment},
    types::{Effect, FunctionFacts}
};
use crate::semantic::{
    expr::{MUTATING_METHODS, compound_assignment},
    knowledge,
    std_knowledge::EXTERNAL_CONSTRUCTORS
};

/// Print-family macros with the stream each one writes to and whether it
/// appends a line terminator. Every entry also carries
/// [`Effect::ConsoleIo`], which stays the coarse summary category.
const OUTPUT_MACROS: [(&str, super::output::Stream, super::output::Newline); 5] = [
    (
        "dbg",
        super::output::Stream::Stderr,
        super::output::Newline::Appended
    ),
    (
        "eprint",
        super::output::Stream::Stderr,
        super::output::Newline::None
    ),
    (
        "eprintln",
        super::output::Stream::Stderr,
        super::output::Newline::Appended
    ),
    (
        "print",
        super::output::Stream::Stdout,
        super::output::Newline::None
    ),
    (
        "println",
        super::output::Stream::Stdout,
        super::output::Newline::Appended
    )
];

/// Macros that write to a destination handed to them as their first
/// argument. Where [`OUTPUT_MACROS`] fix the stream by name, these
/// leave it to the destination, which is a different question and is
/// answered by [`FactCollector::write_destination`].
const WRITER_MACROS: [(&str, super::output::Newline); 2] = [
    ("write", super::output::Newline::None),
    ("writeln", super::output::Newline::Appended)
];

/// The write-trait method that puts a caller-provided byte slice on a
/// writer. It is the one write spelling that carries its payload as a
/// value rather than as a format template, so a payload it cannot read
/// is opaque for a different reason than an unreadable format string.
const BYTE_WRITE_METHOD: &str = "write_all";

/// Standard-library constructors handing out a handle to one of the
/// process's own streams, with the stream each names.
const STREAM_HANDLES: [(&str, super::output::Stream); 2] = [
    ("stderr", super::output::Stream::Stderr),
    ("stdout", super::output::Stream::Stdout)
];

/// Written type names that name one of the process's own streams, so a
/// binding declared as one is a write to that stream.
const STREAM_TYPES: [(&str, super::output::Stream); 4] = [
    ("Stderr", super::output::Stream::Stderr),
    ("StderrLock", super::output::Stream::Stderr),
    ("Stdout", super::output::Stream::Stdout),
    ("StdoutLock", super::output::Stream::Stdout)
];

/// Macros whose every argument is an expression, so the fact channel can
/// read the same tokens the reachability fold reads.
///
/// Deliberately narrower than [`SETTLED_MACROS`]: `matches!` takes a
/// pattern and `stringify!` takes arbitrary tokens, and reading either
/// as an expression would invent call and read facts the source does not
/// carry.
const EXPRESSION_MACROS: [&str; 20] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "eprint",
    "eprintln",
    "format",
    "format_args",
    "panic",
    "print",
    "println",
    "todo",
    "unimplemented",
    "unreachable",
    "vec",
    "write",
    "writeln"
];

/// Iterator and `Option` adapters whose single-parameter closure
/// receives one element of the adapted collection or option.
const ITERATOR_ADAPTERS: [&str; 18] = [
    "all",
    "and_then",
    "any",
    "filter",
    "filter_map",
    "find",
    "find_map",
    "flat_map",
    "for_each",
    "inspect",
    "is_some_and",
    "map",
    "map_or",
    "map_or_else",
    "position",
    "retain",
    "skip_while",
    "take_while"
];

/// Iterator constructors and element accessors that keep the
/// collection's element type.
const ITERATOR_SOURCES: [&str; 7] = [
    "drain",
    "first",
    "get",
    "into_iter",
    "iter",
    "iter_mut",
    "last"
];

/// Macros that end the current path rather than returning to it.
const DIVERGING_MACROS: [&str; 4] = ["panic", "todo", "unimplemented", "unreachable"];

/// Macros whose expansion is known to return to the point that invoked
/// it. A macro outside this list may expand to anything, a divergence
/// included, so its completion is recorded as unproven rather than
/// assumed harmless — the same direction the rest of this channel takes
/// when it cannot see through a construct.
///
/// The list holds standard macros only; a user macro of the same name
/// shadowing one of them would break the reading, which is the standing
/// assumption of every name-keyed recognizer in this tier.
const SETTLED_MACROS: [&str; 28] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "cfg",
    "column",
    "concat",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "env",
    "eprint",
    "eprintln",
    "file",
    "format",
    "format_args",
    "include_bytes",
    "include_str",
    "line",
    "matches",
    "module_path",
    "option_env",
    "print",
    "println",
    "stringify",
    "vec",
    "write",
    "writeln"
];

/// Whether the program point after a construct is reached, given that
/// the construct is entered.
///
/// The three values form a lattice, not a scale: `Never` and `Always`
/// are both proofs and are incomparable with each other, `Unknown` is
/// the absence of either proof. Every strong reading this channel emits
/// — "runs on every path", "can never run" — is a claim about all
/// executions, so it is only ever made from a proof; anything this tier
/// cannot see through lands on `Unknown` and weakens the reading instead
/// of strengthening it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Completion {
    /// The point after the construct is never reached: every path
    /// through it leaves by a return, a break, or a divergence.
    Never,
    /// Neither reaching the point after nor leaving before it is proven.
    Unknown,
    /// The point after the construct is always reached.
    #[default]
    Always
}

impl Completion {
    /// Sequential composition: what holds after running `next` where
    /// `self` completes. A divergence anywhere in the sequence carries
    /// through, an unproven step leaves the whole sequence unproven.
    const fn then(self, next: Self) -> Self {
        match (self, next) {
            (Self::Never, _) | (_, Self::Never) => Self::Never,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Always, Self::Always) => Self::Always
        }
    }

    /// Branch merge: a proof survives only when every branch proves the
    /// same thing, which is what makes a divergence on all arms of an
    /// exhaustive construct kill what follows and a divergence on one
    /// arm merely weaken it.
    const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Never, Self::Never) => Self::Never,
            (Self::Always, Self::Always) => Self::Always,
            _ => Self::Unknown
        }
    }
}

/// The jumps of one kind — `break` or `continue` — a construct may
/// perform, split by what decides which construct absorbs them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Jumps {
    /// Whether an unlabelled jump of this kind may leave the construct;
    /// the nearest enclosing loop absorbs it.
    bare:    bool,
    /// Labels named by jumps of this kind, each absorbed by the
    /// construct carrying that label.
    labeled: BTreeSet<String>
}

impl Jumps {
    /// The jumps of two constructs taken together, whether they run one
    /// after the other or one instead of the other — either way a jump
    /// of either is a jump of the whole.
    fn merged(mut self, other: Self) -> Self {
        self.bare = self.bare || other.bare;
        self.labeled.extend(other.labeled);
        self
    }

    /// Whether a jump of this kind targets the construct carrying
    /// `label`.
    fn target(&self, label: Option<&syn::Label>) -> bool {
        self.bare
            || label.is_some_and(|label| self.labeled.contains(&label.name.ident.to_string()))
    }

    /// Drops the jumps the construct carrying `label` absorbs.
    fn absorbed_by(&mut self, label: Option<&syn::Label>) {
        self.bare = false;
        if let Some(label) = label {
            self.labeled.remove(&label.name.ident.to_string());
        }
    }
}

/// What one construct proves about the program point after it, together
/// with the non-local exits it may perform on the way there.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Flow {
    /// Whether the point after the construct is reached.
    completion:   Completion,
    /// Whether a `return` or a `?` inside may leave the function.
    returns:      bool,
    /// `break`s inside that leave an enclosing loop or labelled block.
    breaks:       Jumps,
    /// `continue`s inside that hand control back to the top of an
    /// enclosing loop.
    continues:    Jumps,
    /// Whether the construct holds something this tier cannot see
    /// through that may itself carry a `break` or a `continue` — an
    /// unexpanded macro, a token stream no rule of this tier reads.
    ///
    /// It is tracked apart from [`Completion::Unknown`] because the two
    /// answer different questions: a call may diverge and still cannot
    /// jump out of the caller's loop, while a macro may expand to a
    /// bare `break`. Conflating them would leave every opaque call
    /// unbounding a loop that nothing breaks out of.
    opaque_jumps: bool
}

impl Flow {
    /// A construct that completes and leaves nothing behind.
    fn settled() -> Self {
        Self::default()
    }

    /// A construct proving `completion` and performing no exit.
    fn proving(completion: Completion) -> Self {
        Self {
            completion,
            ..Self::default()
        }
    }

    /// A construct this tier cannot see through: it proves neither that
    /// the point after it is reached nor that it is not, and whatever it
    /// expands to may carry a `break` or a `continue` out of it.
    fn opaque() -> Self {
        Self {
            completion: Completion::Unknown,
            opaque_jumps: true,
            ..Self::default()
        }
    }

    /// Sequential composition: `next` runs where `self` completes, so a
    /// construct that never completes hides everything after it.
    fn then(self, next: Self) -> Self {
        if matches!(self.completion, Completion::Never) {
            return self;
        }
        Self {
            completion:   self.completion.then(next.completion),
            returns:      self.returns || next.returns,
            breaks:       self.breaks.merged(next.breaks),
            continues:    self.continues.merged(next.continues),
            opaque_jumps: self.opaque_jumps || next.opaque_jumps
        }
    }

    /// Branch merge: control takes one of the two, so an exit either may
    /// perform is possible and only a shared proof survives.
    fn join(self, other: Self) -> Self {
        Self {
            completion:   self.completion.join(other.completion),
            returns:      self.returns || other.returns,
            breaks:       self.breaks.merged(other.breaks),
            continues:    self.continues.merged(other.continues),
            opaque_jumps: self.opaque_jumps || other.opaque_jumps
        }
    }

    /// Weakens a proof of divergence into "not proven either way" and
    /// leaves a proof of completion standing — what a construct that
    /// exists in some builds and not others proves in every build.
    const fn weakened(mut self) -> Self {
        self.completion = self.completion.join(Completion::Always);
        self
    }

    /// Whether the construct may hand control somewhere that skips the
    /// program point after it.
    fn may_skip(&self) -> bool {
        self.returns
            || self.breaks.bare
            || !self.breaks.labeled.is_empty()
            || self.continues.bare
            || !self.continues.labeled.is_empty()
    }

    /// Whether a `break` inside targets the loop that carries `label`.
    fn breaks_out_of(&self, label: Option<&syn::Label>) -> bool {
        self.breaks.target(label)
    }

    /// Whether a `continue` inside restarts the loop that carries
    /// `label`, so the loop can begin another iteration even where the
    /// end of its body is never reached.
    fn continues_into(&self, label: Option<&syn::Label>) -> bool {
        self.continues.target(label)
    }

    /// Drops the jumps a loop absorbs: the unlabelled breaks and
    /// continues, those naming the loop's own label, and the unproven
    /// ones an opaque construct in its body may carry — a bare jump
    /// hidden inside a macro targets the loop that encloses it.
    fn absorbed(mut self, label: Option<&syn::Label>) -> Self {
        self.breaks.absorbed_by(label);
        self.continues.absorbed_by(label);
        self.opaque_jumps = false;
        self
    }
}

/// Whether a repeating construct's body provably leaves on its first
/// pass, so a write inside it runs no more than once per call of the
/// function around it.
///
/// The proof is the body's own completion: a body whose end is never
/// reached cannot start a second iteration. It survives only where
/// nothing can hand control back to the top of the loop — no `continue`
/// this loop absorbs, and no construct opaque enough to hide one.
fn leaves_on_first_pass(body: &Flow, label: Option<&syn::Label>) -> bool {
    matches!(body.completion, Completion::Never)
        && !body.continues_into(label)
        && !body.opaque_jumps
}

/// How many times a write recorded inside a repeating construct's body
/// runs, read off the completion analysis of that body rather than
/// stamped on for standing in a loop.
fn body_multiplicity(body: &Flow, label: Option<&syn::Label>) -> super::output::Multiplicity {
    if leaves_on_first_pass(body, label) {
        super::output::Multiplicity::Once
    } else {
        super::output::Multiplicity::Unbounded
    }
}

/// Attributes written on an expression, for every shape an expression
/// can take. `syn` exposes them per variant only, and a partial match
/// silently loses the gate on the shapes it omits. The variant list is
/// kept one shape per line: a formatter that spreads each pattern over
/// three lines buries the list it is here to make readable.
#[rustfmt::skip]
fn expression_attributes(expression: &syn::Expr) -> &[syn::Attribute] {
    match expression {
        syn::Expr::Array(syn::ExprArray { attrs, .. })
        | syn::Expr::Assign(syn::ExprAssign { attrs, .. })
        | syn::Expr::Async(syn::ExprAsync { attrs, .. })
        | syn::Expr::Await(syn::ExprAwait { attrs, .. })
        | syn::Expr::Binary(syn::ExprBinary { attrs, .. })
        | syn::Expr::Block(syn::ExprBlock { attrs, .. })
        | syn::Expr::Break(syn::ExprBreak { attrs, .. })
        | syn::Expr::Call(syn::ExprCall { attrs, .. })
        | syn::Expr::Cast(syn::ExprCast { attrs, .. })
        | syn::Expr::Closure(syn::ExprClosure { attrs, .. })
        | syn::Expr::Const(syn::ExprConst { attrs, .. })
        | syn::Expr::Continue(syn::ExprContinue { attrs, .. })
        | syn::Expr::Field(syn::ExprField { attrs, .. })
        | syn::Expr::ForLoop(syn::ExprForLoop { attrs, .. })
        | syn::Expr::Group(syn::ExprGroup { attrs, .. })
        | syn::Expr::If(syn::ExprIf { attrs, .. })
        | syn::Expr::Index(syn::ExprIndex { attrs, .. })
        | syn::Expr::Infer(syn::ExprInfer { attrs, .. })
        | syn::Expr::Let(syn::ExprLet { attrs, .. })
        | syn::Expr::Lit(syn::ExprLit { attrs, .. })
        | syn::Expr::Loop(syn::ExprLoop { attrs, .. })
        | syn::Expr::Macro(syn::ExprMacro { attrs, .. })
        | syn::Expr::Match(syn::ExprMatch { attrs, .. })
        | syn::Expr::MethodCall(syn::ExprMethodCall { attrs, .. })
        | syn::Expr::Paren(syn::ExprParen { attrs, .. })
        | syn::Expr::Path(syn::ExprPath { attrs, .. })
        | syn::Expr::Range(syn::ExprRange { attrs, .. })
        | syn::Expr::RawAddr(syn::ExprRawAddr { attrs, .. })
        | syn::Expr::Reference(syn::ExprReference { attrs, .. })
        | syn::Expr::Repeat(syn::ExprRepeat { attrs, .. })
        | syn::Expr::Return(syn::ExprReturn { attrs, .. })
        | syn::Expr::Struct(syn::ExprStruct { attrs, .. })
        | syn::Expr::Try(syn::ExprTry { attrs, .. })
        | syn::Expr::TryBlock(syn::ExprTryBlock { attrs, .. })
        | syn::Expr::Tuple(syn::ExprTuple { attrs, .. })
        | syn::Expr::Unary(syn::ExprUnary { attrs, .. })
        | syn::Expr::Unsafe(syn::ExprUnsafe { attrs, .. })
        | syn::Expr::While(syn::ExprWhile { attrs, .. })
        | syn::Expr::Yield(syn::ExprYield { attrs, .. }) => attrs,
        _ => &[]
    }
}

/// Attributes written on an item, for every shape an item can take.
#[rustfmt::skip]
fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(syn::ItemConst { attrs, .. })
        | syn::Item::Enum(syn::ItemEnum { attrs, .. })
        | syn::Item::ExternCrate(syn::ItemExternCrate { attrs, .. })
        | syn::Item::Fn(syn::ItemFn { attrs, .. })
        | syn::Item::ForeignMod(syn::ItemForeignMod { attrs, .. })
        | syn::Item::Impl(syn::ItemImpl { attrs, .. })
        | syn::Item::Macro(syn::ItemMacro { attrs, .. })
        | syn::Item::Mod(syn::ItemMod { attrs, .. })
        | syn::Item::Static(syn::ItemStatic { attrs, .. })
        | syn::Item::Struct(syn::ItemStruct { attrs, .. })
        | syn::Item::Trait(syn::ItemTrait { attrs, .. })
        | syn::Item::TraitAlias(syn::ItemTraitAlias { attrs, .. })
        | syn::Item::Type(syn::ItemType { attrs, .. })
        | syn::Item::Union(syn::ItemUnion { attrs, .. })
        | syn::Item::Use(syn::ItemUse { attrs, .. }) => attrs,
        _ => &[]
    }
}

/// Attributes written on a statement, whatever shape it takes.
fn statement_attributes(statement: &syn::Stmt) -> &[syn::Attribute] {
    match statement {
        syn::Stmt::Local(local) => &local.attrs,
        syn::Stmt::Item(item) => item_attributes(item),
        syn::Stmt::Expr(expression, _) => expression_attributes(expression),
        syn::Stmt::Macro(invocation) => &invocation.attrs
    }
}

/// Returns whether an attribute can remove the statement it stands on
/// from a build.
///
/// `cfg` can. `cfg_attr` cannot: it applies an *attribute*
/// conditionally to a statement that is present in every build, so
/// reading it as a gate would weaken an unconditional write and let a
/// genuine divergence stop killing what follows it. The one spelling
/// that does gate is `cfg_attr` applying a `cfg`, which is read here
/// rather than assumed away. An attribute whose arguments do not parse
/// keeps the weakening, the direction that claims less.
fn configuration_gate(attribute: &syn::Attribute) -> bool {
    if attribute.path().is_ident("cfg") {
        return true;
    }
    if !attribute.path().is_ident("cfg_attr") {
        return false;
    }
    let Ok(applied) = attribute.parse_args_with(
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
    ) else {
        return true;
    };
    applied
        .iter()
        .skip(1)
        .any(|meta| meta.path().is_ident("cfg"))
}

/// Returns whether a statement is gated by a configuration attribute
/// this tier does not resolve.
fn gated(statement: &syn::Stmt) -> bool {
    statement_attributes(statement)
        .iter()
        .any(configuration_gate)
}

/// Renders a path as segment names, the first segment substituted with
/// its full `use` path when the file imported it.
fn expanded_segments(imports: &BTreeMap<String, String>, path: &syn::Path) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    for (position, segment) in path.segments.iter().enumerate() {
        let name = segment.ident.to_string();
        if position == 0 {
            match imports.get(&name) {
                Some(full) => segments.extend(full.split("::").map(str::to_owned)),
                None => segments.push(name)
            }
        } else {
            segments.push(name);
        }
    }
    segments
}

/// The context the reachability fold reads beyond the syntax it walks.
///
/// It carries the enclosing file's `use` table, and that is the reason
/// the fold is a set of methods rather than free functions: a name is
/// only a path once the imports have been applied to it, and a
/// recognizer that skips that step disagrees with every other
/// recognizer in this module about the same call.
struct FlowScope<'a> {
    /// Imported names of the enclosing file to their full `use` paths.
    imports: &'a BTreeMap<String, String>,
    /// Workspace functions to their return types, so a call to a
    /// function whose signature declares the never type is read as the
    /// divergence it is.
    returns: &'a Scoped<Returned>,
    /// Source index of the function under analysis, for scoped lookup.
    src:     usize,
    /// Crate identity of every source, for scoped lookup.
    crates:  &'a [String]
}

impl FlowScope<'_> {
    /// Returns whether a call expression names the process-ending pair,
    /// the file's imports applied first — `use std::process::exit` and
    /// `std::process::exit` are the same call and are read the same way.
    fn process_exit(&self, callee: &syn::Expr) -> bool {
        let syn::Expr::Path(path) = callee else {
            return false;
        };
        let segments = expanded_segments(self.imports, &path.path);
        matches!(segments.last().map(String::as_str), Some("exit" | "abort"))
            && segments.iter().any(|segment| segment == "process")
    }

    /// Whether a called path names a workspace function whose signature
    /// declares the never type, so the call does not return.
    fn never_returns(&self, callee: &syn::Expr) -> bool {
        let syn::Expr::Path(path) = callee else {
            return false;
        };
        let segments = expanded_segments(self.imports, &path.path);
        let Some(bare) = segments.last() else {
            return false;
        };
        let qualified = if segments.len() >= 2 {
            format!("{}::{bare}", segments[segments.len() - 2])
        } else {
            bare.clone()
        };
        [qualified.as_str(), bare.as_str()]
            .iter()
            .filter_map(|key| self.returns.resolve(key, self.src, self.crates))
            .any(|returned| returned.never)
    }

    /// What a statement proves about the statement after it. A
    /// configuration gate weakens the answer and never decides it: the
    /// gated statement exists in some builds and not in others, so a
    /// divergence under a gate proves nothing about a build where the
    /// gate is off.
    fn statement_flow(&self, statement: &syn::Stmt) -> Flow {
        let flow = match statement {
            syn::Stmt::Local(local) => self.local_flow(local),
            syn::Stmt::Item(_) => Flow::settled(),
            syn::Stmt::Expr(expression, _) => self.expression_flow(expression),
            syn::Stmt::Macro(invocation) => self.macro_flow(&invocation.mac)
        };
        if gated(statement) {
            flow.weakened()
        } else {
            flow
        }
    }

    /// What a `let` proves: its initializer runs, and a `let` … `else`
    /// that binds by pattern reaches the next statement only when the
    /// pattern matched, which no syntax proves.
    fn local_flow(&self, local: &syn::Local) -> Flow {
        let Some(init) = &local.init else {
            return Flow::settled();
        };
        let flow = self.expression_flow(&init.expr);
        let Some((_, otherwise)) = &init.diverge else {
            return flow;
        };
        let mut escape = self.expression_flow(otherwise);
        escape.completion = Completion::Unknown;
        flow.then(escape)
    }

    /// Left fold of the statements of a block.
    fn block_flow(&self, block: &syn::Block) -> Flow {
        let mut flow = Flow::settled();
        for statement in &block.stmts {
            flow = flow.then(self.statement_flow(statement));
        }
        flow
    }

    /// What a macro invocation proves. A macro that ends the path is
    /// proven to; a macro whose expansion this tier knows folds over its
    /// arguments, which is where a `?` or a diverging call inside one is
    /// seen; anything else could expand to a divergence — or to a bare
    /// `break` — and is recorded as unproven on both counts.
    fn macro_flow(&self, invocation: &syn::Macro) -> Flow {
        let Some(name) = invocation
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
        else {
            return Flow::opaque();
        };
        if DIVERGING_MACROS.contains(&name.as_str()) {
            return Flow::proving(Completion::Never);
        }
        if !SETTLED_MACROS.contains(&name.as_str()) {
            return Flow::opaque();
        }
        let Ok(arguments) = invocation.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        ) else {
            return Flow::settled();
        };
        let mut flow = Flow::settled();
        for argument in &arguments {
            flow = flow.then(self.expression_flow(argument));
        }
        flow
    }

    /// What a conditional proves: the branch not taken is as real as the
    /// one taken, and a missing `else` branch is a branch that
    /// completes.
    fn conditional_flow(&self, conditional: &syn::ExprIf) -> Flow {
        let condition = self.expression_flow(&conditional.cond);
        let taken = self.block_flow(&conditional.then_branch);
        let otherwise = conditional
            .else_branch
            .as_ref()
            .map_or_else(Flow::settled, |(_, branch)| self.expression_flow(branch));
        condition.then(taken.join(otherwise))
    }

    /// What a match proves: control enters exactly one arm, so the arms
    /// merge. A match with no arms is matching a value that cannot exist
    /// and never completes.
    fn match_flow(&self, matched: &syn::ExprMatch) -> Flow {
        let scrutinee = self.expression_flow(&matched.expr);
        let mut arms: Option<Flow> = None;
        for arm in &matched.arms {
            let condition = arm_condition(&arm.pat).map_or_else(Flow::settled, |guard| {
                self.expression_flow(guard).weakened()
            });
            let taken = condition.then(self.expression_flow(&arm.body));
            arms = Some(match arms {
                Some(seen) => seen.join(taken),
                None => taken
            });
        }
        scrutinee.then(arms.unwrap_or_else(|| Flow::proving(Completion::Never)))
    }

    /// Sequential fold over the immediate children of a construct whose
    /// own control flow needs no rule of its own — every child runs, in
    /// written order. Nested items are not children of the flow: their
    /// bodies run where they are called, not here.
    fn children_flow(&self, expression: &syn::Expr) -> Flow {
        struct Children<'scope, 'imports> {
            flow:  Flow,
            scope: &'scope FlowScope<'imports>
        }
        impl<'ast> Visit<'ast> for Children<'_, '_> {
            fn visit_expr(&mut self, child: &'ast syn::Expr) {
                self.flow = std::mem::take(&mut self.flow).then(self.scope.expression_flow(child));
            }

            fn visit_block(&mut self, child: &'ast syn::Block) {
                self.flow = std::mem::take(&mut self.flow).then(self.scope.block_flow(child));
            }

            fn visit_item(&mut self, _item: &'ast syn::Item) {}
        }
        let mut children = Children {
            flow:  Flow::settled(),
            scope: self
        };
        syn::visit::visit_expr(&mut children, expression);
        children.flow
    }

    /// What a closure invoked where it is written proves: its body runs
    /// here, so a divergence in it ends this path. A `return` inside
    /// leaves the closure and not the function around it, which no
    /// syntax here tells apart from a body that falls off its end, so a
    /// returning body proves nothing either way.
    fn invoked_body_flow(&self, closure: &syn::ExprClosure) -> Flow {
        let body = self.expression_flow(&closure.body);
        let completion = if body.returns {
            body.completion.join(Completion::Always)
        } else {
            body.completion
        };
        Flow {
            completion,
            opaque_jumps: body.opaque_jumps,
            ..Flow::default()
        }
    }

    /// What an expression proves about the program point after it.
    ///
    /// The rules are the syntax-directed reachability fold: every
    /// construct that can end a path has one, and everything else folds
    /// over its children, so a divergence nested in a branch, a block, a
    /// match arm or a call argument is seen instead of being missed for
    /// sitting one level below the top of a statement.
    fn expression_flow(&self, expression: &syn::Expr) -> Flow {
        match expression {
            syn::Expr::Return(returned) => {
                let value = returned
                    .expr
                    .as_deref()
                    .map_or_else(Flow::settled, |value| self.expression_flow(value));
                let mut flow = value.then(Flow::proving(Completion::Never));
                flow.returns = true;
                flow
            }
            syn::Expr::Break(broken) => {
                let value = broken
                    .expr
                    .as_deref()
                    .map_or_else(Flow::settled, |value| self.expression_flow(value));
                let mut flow = value.then(Flow::proving(Completion::Never));
                if let Some(label) = &broken.label {
                    flow.breaks.labeled.insert(label.ident.to_string());
                } else {
                    flow.breaks.bare = true;
                }
                flow
            }
            syn::Expr::Continue(continued) => {
                let mut flow = Flow::proving(Completion::Never);
                if let Some(label) = &continued.label {
                    flow.continues.labeled.insert(label.ident.to_string());
                } else {
                    flow.continues.bare = true;
                }
                flow
            }
            syn::Expr::Try(tried) => {
                let mut flow = self
                    .expression_flow(&tried.expr)
                    .then(Flow::proving(Completion::Unknown));
                flow.returns = true;
                flow
            }
            syn::Expr::Macro(invocation) => self.macro_flow(&invocation.mac),
            syn::Expr::Call(call) => {
                let flow = self.children_flow(expression);
                if self.process_exit(&call.func) || self.never_returns(&call.func) {
                    return flow.then(Flow::proving(Completion::Never));
                }
                if let syn::Expr::Closure(closure) = unwrapped(&call.func) {
                    return flow.then(self.invoked_body_flow(closure));
                }
                flow
            }
            syn::Expr::If(conditional) => self.conditional_flow(conditional),
            syn::Expr::Match(matched) => self.match_flow(matched),
            syn::Expr::Loop(repeated) => {
                let body = self.block_flow(&repeated.body);
                let escapes = body.breaks_out_of(repeated.label.as_ref()) || body.opaque_jumps;
                let mut flow = body.absorbed(repeated.label.as_ref());
                flow.completion = if escapes {
                    Completion::Unknown
                } else {
                    Completion::Never
                };
                flow
            }
            syn::Expr::While(conditional) => {
                let condition = self.expression_flow(&conditional.cond);
                let mut body = self
                    .block_flow(&conditional.body)
                    .absorbed(conditional.label.as_ref());
                body.completion = Completion::Always;
                condition.then(body)
            }
            syn::Expr::ForLoop(iterated) => {
                let iterable = self.expression_flow(&iterated.expr);
                let mut body = self
                    .block_flow(&iterated.body)
                    .absorbed(iterated.label.as_ref());
                body.completion = Completion::Always;
                iterable.then(body)
            }
            syn::Expr::Closure(_) | syn::Expr::Async(_) | syn::Expr::Const(_) => Flow::settled(),
            syn::Expr::Block(braced) => {
                let mut flow = self.block_flow(&braced.block);
                if let Some(label) = &braced.label
                    && flow.breaks.labeled.remove(&label.name.ident.to_string())
                {
                    flow.completion = flow.completion.join(Completion::Always);
                }
                flow
            }
            syn::Expr::Binary(binary)
                if matches!(binary.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) =>
            {
                self.expression_flow(&binary.left)
                    .then(self.expression_flow(&binary.right).weakened())
            }
            syn::Expr::Verbatim(_) => Flow::opaque(),
            _ => self.children_flow(expression)
        }
    }
}

/// The extra condition an arm carries, which its pattern holds: an arm
/// guard is a `Pat::Guard` wrapping the pattern it qualifies.
fn arm_condition(pattern: &syn::Pat) -> Option<&syn::Expr> {
    match pattern {
        syn::Pat::Guard(guarded) => Some(&guarded.guard),
        _ => None
    }
}

/// Returns whether a pattern accepts every value reaching it, so an arm
/// carrying it binds rather than branches. A binding with a subpattern —
/// `n @ 1..=5` — is refutable: the subpattern decides, not the binding.
/// A guard is not part of this answer; it is recorded beside it.
fn irrefutable(pattern: &syn::Pat) -> bool {
    match pattern {
        syn::Pat::Wild(_) => true,
        syn::Pat::Ident(binding) => binding.subpat.is_none(),
        syn::Pat::Guard(guarded) => irrefutable(&guarded.pat),
        syn::Pat::Paren(parenthesized) => irrefutable(&parenthesized.pat),
        syn::Pat::Reference(borrowed) => irrefutable(&borrowed.pat),
        syn::Pat::Type(typed) => irrefutable(&typed.pat),
        syn::Pat::Tuple(tuple) => tuple.elems.iter().all(irrefutable),
        _ => false
    }
}

/// Syntax visitor accumulating facts from one function body.
pub(super) struct FactCollector {
    /// Facts collected so far.
    pub(super) facts:       FunctionFacts,
    /// Parameter name to type-name mapping from the signature.
    pub(super) param_types: BTreeMap<String, String>,
    /// Binding name to collection element type, from the signature.
    pub(super) elem_types:  BTreeMap<String, String>,
    /// Workspace functions to their return types, scoped by source.
    pub(super) fn_returns:  Scoped<Returned>,
    /// `Type.field` keys to field type names, scoped by source.
    pub(super) field_types: Scoped<String>,
    /// `Type.field` keys of collection fields to element types.
    pub(super) field_elems: Scoped<String>,
    /// `Type.field` keys of wrapper fields to their wrapped inner type.
    pub(super) field_wraps: Scoped<String>,
    /// Crate identity of every source, for crate-proximity resolution.
    pub(super) crates:      Vec<String>,
    /// Source index of the function under analysis.
    pub(super) src:         usize,
    /// Enclosing impl block's generic parameters to their first trait
    /// bound, so a generic-typed field resolves as its bound.
    pub(super) bounds:      BTreeMap<String, String>,
    /// Let bindings holding a wrapper value with a known wrapped inner
    /// type, cloned or borrowed off a wrapper field.
    pub(super) local_wraps: BTreeMap<String, String>,
    /// Imported names of the enclosing file to their full `use` paths.
    pub(super) imports:     BTreeMap<String, String>,
    /// Let bindings holding a value provably constructed in this body.
    pub(super) fresh:       BTreeSet<String>,
    /// Constructs currently enclosing the visited node, outermost first.
    pub(super) guards:      Vec<super::output::Guard>,
    /// Exits that may leave the function before the visited node.
    pub(super) prior_exits: u32,
    /// Whether the visited node follows an unconditional divergence.
    pub(super) dead:        bool
}

impl FactCollector {
    /// Resolves a return-table key for this function's scope.
    fn ret(&self, key: &str) -> Option<&Returned> {
        self.fn_returns.resolve(key, self.src, &self.crates)
    }

    /// Resolves a field-type key for this function's scope.
    fn field(&self, key: &str) -> Option<&String> {
        self.field_types.resolve(key, self.src, &self.crates)
    }

    /// Resolves a field element-type key for this function's scope.
    fn elem(&self, key: &str) -> Option<&String> {
        self.field_elems.resolve(key, self.src, &self.crates)
    }

    /// Resolves a wrapper inner-type key for this function's scope.
    fn wrap(&self, key: &str) -> Option<&String> {
        self.field_wraps.resolve(key, self.src, &self.crates)
    }

    /// Renders a field expression as `Owner.field` when the base is a
    /// typed parameter or `self`, or `?.field` otherwise.
    fn field_key(&self, field: &syn::ExprField) -> Option<String> {
        let syn::Member::Named(name) = &field.member else {
            return None;
        };
        let owner = match field.base.as_ref() {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map_or("?", String::as_str),
            _ => "?"
        };
        Some(format!("{owner}.{name}"))
    }

    /// Records the outermost field of an assignment target as a write —
    /// or as a build when the base is a freshly constructed local — and
    /// counts the base parameter's use so a written-through parameter
    /// never looks read-only.
    fn record_write_target(&mut self, target: &syn::Expr) {
        if let syn::Expr::Field(field) = target {
            if let Some(key) = self.field_key(field) {
                self.record_field_write(field, key);
            }
            if let syn::Expr::Path(path) = field.base.as_ref()
                && let Some(ident) = path.path.get_ident()
            {
                self.bump_param(&ident.to_string(), false);
            }
        }
    }

    /// Routes one field write to `builds` when its base binding holds a
    /// value constructed in this body, to `writes` otherwise.
    fn record_field_write(&mut self, field: &syn::ExprField, key: String) {
        let fresh_base = matches!(
            field.base.as_ref(),
            syn::Expr::Path(path) if path
                .path
                .get_ident()
                .is_some_and(|ident| self.fresh.contains(&ident.to_string()))
        );
        if fresh_base {
            self.facts.builds.push(key);
        } else {
            self.facts.writes.push(key);
        }
    }

    /// Counts one use of the parameter named `name`; `field_read` marks a
    /// use that is only a field read.
    fn bump_param(&mut self, name: &str, field_read: bool) {
        if let Some(param) = self
            .facts
            .params
            .iter_mut()
            .find(|param| param.name == name)
        {
            param.uses += 1;
            if field_read {
                param.field_reads += 1;
            }
        }
    }

    /// Element type of an iterable expression: a binding or field of a
    /// known collection type, seen through references, parentheses, and
    /// the plain iterator constructors.
    fn element_of(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Reference(reference) => self.element_of(&reference.expr),
            syn::Expr::Paren(inner) => self.element_of(&inner.expr),
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.elem_types.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => self
                .field_key(field)
                .and_then(|key| self.elem(&key))
                .cloned(),
            syn::Expr::MethodCall(call)
                if ITERATOR_SOURCES.contains(&call.method.to_string().as_str()) =>
            {
                self.element_of(&call.receiver)
            }
            _ => None
        }
    }

    /// Element type behind an `Option`-returning collection accessor —
    /// `get`, `first`, `last` — on a known element type.
    fn optional_element(&self, expression: &syn::Expr) -> Option<String> {
        let syn::Expr::MethodCall(call) = expression else {
            return None;
        };
        if !matches!(call.method.to_string().as_str(), "get" | "first" | "last") {
            return None;
        }
        self.element_of(&call.receiver)
    }

    /// Binds one loop or closure pattern against the iterated element
    /// type, seeing through `enumerate`'s index tuple.
    fn bind_element(&mut self, pattern: &syn::Pat, iterable: &syn::Expr) {
        if let syn::Expr::MethodCall(call) = iterable
            && call.method == "enumerate"
        {
            if let syn::Pat::Tuple(tuple) = pattern
                && tuple.elems.len() == 2
                && let (Some(name), Some(element)) = (
                    binding_name(&tuple.elems[1]),
                    self.element_of(&call.receiver)
                )
            {
                self.param_types.insert(name, element);
            }
            return;
        }
        if let (Some(name), Some(element)) = (binding_name(pattern), self.element_of(iterable)) {
            self.param_types.insert(name, element);
        }
    }

    /// Wrapped inner type of a wrapper expression: a field with a
    /// recorded single-generic inner, a binding that inherited one, or
    /// a `clone` of either — references and derefs stripped.
    fn wrapped_of(&self, expression: &syn::Expr) -> Option<String> {
        match strip_ref_deref(expression) {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.local_wraps.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => {
                let owner = self.expr_type(&field.base)?;
                self.wrap(&format!("{owner}.{}", member_name(&field.member)))
                    .cloned()
            }
            syn::Expr::MethodCall(call) if call.method == "clone" => {
                self.wrapped_of(&call.receiver)
            }
            _ => None
        }
    }

    /// Types the parameter of a single-parameter closure argument with
    /// the receiver field's wrapped inner type — the `do_io(|inner|
    /// inner.read(buf))` shape, where the wrapper hands its wrapped
    /// value to the closure. The same class of approximation as the
    /// iterator-adapter element binding, and wrong the same way a
    /// combinator passing something other than the wrapped value would
    /// make it; single-generic wrappers keep that surface small.
    fn bind_wrapped_closure(&mut self, call: &syn::ExprMethodCall) {
        let Some(inner) = self.wrapped_of(&call.receiver) else {
            return;
        };
        for argument in &call.args {
            if let syn::Expr::Closure(closure) = argument
                && closure.inputs.len() == 1
                && let Some(pattern) = closure.inputs.first()
                && let Some(name) = binding_name(pattern)
            {
                self.param_types.insert(name, inner.clone());
            }
        }
    }

    /// Normalizes a call expression into its return-table key, seeing
    /// through `.await` and preferring the owner-qualified associated
    /// key over the bare free-function name.
    fn call_key(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Await(awaited) => self.call_key(&awaited.base),
            syn::Expr::Call(call) => {
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let segments = &path.path.segments;
                let name = segments.last()?.ident.to_string();
                if segments.len() >= 2 {
                    let written = segments[segments.len() - 2].ident.to_string();
                    let owner = if written == "Self" {
                        self.param_types.get("self").cloned().unwrap_or(written)
                    } else {
                        written
                    };
                    let qualified = format!("{owner}::{name}");
                    if self.ret(&qualified).is_some() {
                        return Some(qualified);
                    }
                }
                self.ret(&name).is_some().then_some(name)
            }
            _ => None
        }
    }

    /// Types any expression the tables can settle: bindings and fields
    /// by their recorded types, indexed collections by element, calls
    /// through [`Self::called_return`].
    fn expr_type(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map(|name| self.bounds.get(name).unwrap_or(name).clone()),
            syn::Expr::Field(field) => {
                let owner = self.expr_type(&field.base)?;
                self.field(&format!("{owner}.{}", member_name(&field.member)))
                    .map(|inner| self.bounds.get(inner).unwrap_or(inner).clone())
            }
            syn::Expr::Index(indexed) => self.element_of(&indexed.expr),
            other => self.called_return(other)
        }
    }

    /// Types an expression that calls a function with a known return: a
    /// `?`- or `unwrap`/`expect`-unwrapped call yields the success
    /// type, a plain call the written return type, a method chain
    /// resolves receiver-first, and a well-known external constructor
    /// keeps its external type name so calls on it attribute outside.
    fn called_return(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Try(tried) => self
                .call_key(&tried.expr)
                .and_then(|key| self.ret(&key))
                .and_then(|returned| returned.success.clone())
                .or_else(|| self.optional_element(&tried.expr)),
            syn::Expr::MethodCall(call) if call.method == "unwrap" || call.method == "expect" => {
                if let Some(key) = self.call_key(&call.receiver) {
                    return self.ret(&key).and_then(|returned| returned.success.clone());
                }
                self.optional_element(&call.receiver)
                    .or_else(|| initializer_type(&call.receiver))
                    .or_else(|| self.called_return(&call.receiver))
            }
            syn::Expr::Await(awaited) => self.called_return(&awaited.base),
            syn::Expr::MethodCall(call) => {
                let owner = self.expr_type(&call.receiver)?;
                let method = call.method.to_string();
                if method == "clone" {
                    return Some(owner);
                }
                let declared = self
                    .ret(&format!("{owner}::{method}"))
                    .map(|returned| returned.plain.clone());
                if knowledge::PROJECTION_METHODS.contains(&method.as_str())
                    && declared
                        .as_deref()
                        .is_none_or(|view| knowledge::PROJECTION_VIEWS.contains(&view))
                {
                    return Some(owner);
                }
                declared.or_else(|| knowledge::method_return(&owner, &method).map(str::to_owned))
            }
            syn::Expr::Call(call) => {
                if let Some(inner) = pinned_inner(call) {
                    return self.expr_type(inner);
                }
                if let Some(key) = self.call_key(expression) {
                    return self.ret(&key).map(|returned| returned.plain.clone());
                }
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let name = path.path.segments.last()?.ident.to_string();
                EXTERNAL_CONSTRUCTORS
                    .iter()
                    .find(|(constructor, _)| *constructor == name)
                    .map(|(_, produced)| (*produced).to_owned())
                    .or_else(|| knowledge::constructor_type(&name).map(str::to_owned))
            }
            _ => self
                .call_key(expression)
                .and_then(|key| self.ret(&key))
                .map(|returned| returned.plain.clone())
        }
    }

    /// Renders a path as segment names, the first segment substituted
    /// with its full `use` path when the file imported it — so a bare
    /// `remove_dir_all` imported from `std::fs` classifies as a
    /// filesystem sink.
    fn expanded_segments(&self, path: &syn::Path) -> Vec<String> {
        expanded_segments(&self.imports, path)
    }

    /// The reachability fold read against this function's own file.
    fn flow_scope(&self) -> FlowScope<'_> {
        FlowScope {
            imports: &self.imports,
            returns: &self.fn_returns,
            src:     self.src,
            crates:  &self.crates
        }
    }

    /// Classifies a call path against the effect sink patterns, seen
    /// through the file's imports.
    fn record_path_effect(&mut self, path: &syn::Path) {
        let segments = self.expanded_segments(path);
        let joined = segments.join("::");
        let effect = if joined.contains("fs::") || segments.first().is_some_and(|s| s == "File") {
            Some(Effect::FileSystem)
        } else if joined.contains("net::")
            || segments
                .iter()
                .any(|s| s == "TcpStream" || s == "TcpListener" || s == "UdpSocket")
        {
            Some(Effect::Network)
        } else if joined.contains("process::") || segments.first().is_some_and(|s| s == "Command")
        {
            Some(Effect::Process)
        } else if joined.contains("env::") {
            Some(Effect::Environment)
        } else if segments.len() == 2 {
            segments
                .first()
                .and_then(|first| knowledge::crate_effect(first))
        } else {
            None
        };
        if let Some(effect) = effect {
            self.facts.effects.push(effect);
        }
    }

    /// Visits `body` with one more enclosing construct on the stack, so
    /// every write recorded inside it carries that construct.
    fn under<F: FnOnce(&mut Self)>(&mut self, guard: super::output::Guard, body: F) {
        self.guards.push(guard);
        body(self);
        self.guards.pop();
    }

    /// Visits `body` as the body of a repeating construct, stamping
    /// every write recorded inside it with `multiplicity`.
    ///
    /// Repetition is recorded here rather than read off the guard stack,
    /// because repeating and weakening are different questions: a `loop`
    /// repeats without bound while its body is still entered
    /// unconditionally. It is also not stamped for standing inside a
    /// loop: `multiplicity` comes from the completion analysis of the
    /// body, so a body that provably leaves on its first pass keeps the
    /// single run it has.
    fn repeating<F: FnOnce(&mut Self)>(
        &mut self,
        multiplicity: super::output::Multiplicity,
        body: F
    ) {
        let recorded = self.facts.output_writes.len();
        body(self);
        if matches!(multiplicity, super::output::Multiplicity::Once) {
            return;
        }
        for write in self.facts.output_writes.iter_mut().skip(recorded) {
            write.multiplicity = multiplicity;
        }
    }

    /// Snapshot of everything standing between the current position and
    /// unconditional execution.
    fn site_guard(&self) -> super::output::SiteGuard {
        super::output::SiteGuard {
            guards:      self.guards.clone(),
            prior_exits: self.prior_exits,
            dead:        self.dead
        }
    }

    /// Records one write to an output stream, in body order.
    ///
    /// The payload is whatever the construct proves and no more; the
    /// byte range of the construct is kept so a claim can point at what
    /// produced it.
    fn push_output(
        &mut self,
        span: std::ops::Range<usize>,
        stream: super::output::Stream,
        payload: super::output::Payload,
        newline: super::output::Newline
    ) {
        self.facts.output_writes.push(super::output::OutputWrite {
            stream,
            payload,
            newline,
            site: rustmanifest_schema::Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            },
            guard: self.site_guard(),
            multiplicity: super::output::Multiplicity::Once
        });
    }

    /// Records one write by a print-family macro, whose stream is fixed
    /// by the macro's own name.
    fn record_output(
        &mut self,
        invocation: &syn::Macro,
        stream: super::output::Stream,
        newline: super::output::Newline,
        debug_macro: bool
    ) {
        use syn::spanned::Spanned;
        self.push_output(
            invocation.span().byte_range(),
            stream,
            super::output::macro_payload(invocation, debug_macro),
            newline
        );
    }

    /// Records one write by a macro that takes its destination as the
    /// first argument. The destination decides the stream, the remaining
    /// arguments carry the payload, and an invocation whose arguments do
    /// not parse is still recorded — with both answers stated as
    /// unproven rather than dropped.
    fn record_writer_output(&mut self, invocation: &syn::Macro, newline: super::output::Newline) {
        use syn::spanned::Spanned;
        let span = invocation.span().byte_range();
        let unreadable = super::output::Payload::Opaque(super::output::OpaqueReason::Unparsed);
        let Ok(arguments) = invocation.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        ) else {
            self.push_output(span, super::output::Stream::Unknown, unreadable, newline);
            return;
        };
        let written: Vec<&syn::Expr> = arguments.iter().collect();
        let Some((destination, rest)) = written.split_first() else {
            self.push_output(span, super::output::Stream::Unknown, unreadable, newline);
            return;
        };
        let stream = self.write_destination(destination);
        let payload = super::output::argument_payload(rest);
        self.push_output(span, stream, payload, newline);
    }

    /// Records one write by the byte-slice write method of the write
    /// trait: the receiver decides the stream, and the bytes are proven
    /// only where they are written as a literal.
    fn record_byte_output(&mut self, call: &syn::ExprMethodCall) {
        use syn::spanned::Spanned;
        let stream = self.write_destination(&call.receiver);
        let payload = call.args.first().map_or(
            super::output::Payload::Empty,
            super::output::literal_bytes_payload
        );
        self.push_output(
            call.span().byte_range(),
            stream,
            payload,
            super::output::Newline::None
        );
    }

    /// The stream a written-to destination names.
    ///
    /// Two questions, asked in order and neither guessed: whether the
    /// destination is built from one of the process's own handles, and
    /// whether it arrives from outside this function — a writer this
    /// function is handed is the caller's to point wherever it likes.
    /// Anything else is recorded as unidentified rather than assumed.
    fn write_destination(&self, destination: &syn::Expr) -> super::output::Stream {
        if let Some(stream) = handle_stream(destination) {
            return stream;
        }
        let syn::Expr::Path(path) = destination_root(destination) else {
            return super::output::Stream::Unknown;
        };
        let Some(name) = path.path.get_ident().map(ToString::to_string) else {
            return super::output::Stream::Unknown;
        };
        if let Some(stream) = self
            .param_types
            .get(&name)
            .and_then(|type_name| named_stream(type_name))
        {
            return stream;
        }
        if self.facts.params.iter().any(|param| param.name == name) {
            return super::output::Stream::CallerSupplied;
        }
        super::output::Stream::Unknown
    }

    /// Visits the arguments of a macro whose arguments are expressions,
    /// so a fact standing inside one is recorded rather than dropped for
    /// the depth it sits at. The reachability fold already reads these
    /// same tokens; reading them for one channel and not the other is
    /// what let a write inside a macro argument disappear.
    fn visit_macro_arguments(&mut self, invocation: &syn::Macro) {
        let Ok(arguments) = invocation.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        ) else {
            return;
        };
        for argument in &arguments {
            Visit::visit_expr(self, argument);
        }
    }

    /// Records one normalized call candidate, resolving `Self` and typed
    /// receivers through the local type map.
    fn record_call(&mut self, owner: Option<&str>, callee: &str) {
        let candidate = match owner {
            None => callee.to_owned(),
            Some("Self") => self.param_types.get("self").map_or_else(
                || format!("Self::{callee}"),
                |own| format!("{own}::{callee}")
            ),
            Some(owner) => format!("{owner}::{callee}")
        };
        self.facts.call_sites.push(super::types::CallSite {
            candidate: candidate.clone(),
            guard:     self.site_guard()
        });
        self.facts.calls.push(candidate);
    }
}

/// Inner expression of a `Pin::new(&mut *value)` call: `Pin` is a
/// transparent pointer for receiver typing, so the pinned value's type
/// is the wrapped expression's type, references and derefs stripped.
fn pinned_inner(call: &syn::ExprCall) -> Option<&syn::Expr> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let segments = &path.path.segments;
    if segments.len() < 2
        || segments[segments.len() - 1].ident != "new"
        || segments[segments.len() - 2].ident != "Pin"
    {
        return None;
    }
    call.args.first().map(strip_ref_deref)
}

/// The process stream a destination expression is built from: the
/// handle constructors, seen through the borrows and the locking calls
/// that hand out a guard over the same stream.
///
/// A buffering wrapper is deliberately not seen through. It is a
/// different destination: bytes handed to it reach the process stream
/// when the buffer is flushed and not when they are written, which is
/// the whole point of putting one there. Claiming otherwise would name
/// the canonical form as a write to a line-buffered handle.
fn handle_stream(expression: &syn::Expr) -> Option<super::output::Stream> {
    match expression {
        syn::Expr::Reference(reference) => handle_stream(&reference.expr),
        syn::Expr::Paren(inner) => handle_stream(&inner.expr),
        syn::Expr::Group(inner) => handle_stream(&inner.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            handle_stream(&unary.expr)
        }
        syn::Expr::MethodCall(call) => handle_stream(&call.receiver),
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let name = path.path.segments.last()?.ident.to_string();
            STREAM_HANDLES
                .iter()
                .find(|(handle, _)| *handle == name.as_str())
                .map(|(_, stream)| *stream)
        }
        _ => None
    }
}

/// The process stream a written type name stands for.
fn named_stream(type_name: &str) -> Option<super::output::Stream> {
    STREAM_TYPES
        .iter()
        .find(|(named, _)| *named == type_name)
        .map(|(_, stream)| *stream)
}

/// The expression inside the parentheses and invisible groupings around
/// it — a closure invoked where it is written has to be parenthesized,
/// so a recognizer that skips them never sees one.
fn unwrapped(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Paren(inner) => unwrapped(&inner.expr),
        syn::Expr::Group(inner) => unwrapped(&inner.expr),
        _ => expression
    }
}

/// The expression a write destination is ultimately reached from,
/// borrows, parentheses and the method chain over it stripped.
fn destination_root(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(reference) => destination_root(&reference.expr),
        syn::Expr::Paren(inner) => destination_root(&inner.expr),
        syn::Expr::Group(inner) => destination_root(&inner.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            destination_root(&unary.expr)
        }
        syn::Expr::MethodCall(call) => destination_root(&call.receiver),
        _ => expression
    }
}

/// Strips leading references and dereferences off an expression.
fn strip_ref_deref(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(reference) => strip_ref_deref(&reference.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            strip_ref_deref(&unary.expr)
        }
        _ => expression
    }
}

/// Table key of a field member: the written name, or the position for
/// the unnamed fields of a tuple struct — the shape newtype wrappers
/// carry their payload in. Used by the typing channel only; the
/// read/write fact channel keeps naming its members as written.
fn member_name(member: &syn::Member) -> String {
    match member {
        syn::Member::Named(name) => name.to_string(),
        syn::Member::Unnamed(index) => index.index.to_string()
    }
}

/// Name of a single binding pattern, seen through references and type
/// ascriptions.
fn binding_name(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Reference(reference) => binding_name(&reference.pat),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        _ => None
    }
}

/// Infers the type name of a `let` initializer: a struct literal or a
/// `Type::new(..)` / `Type::default()` constructor call.
fn initializer_type(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Struct(literal) => literal
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let segments = &path.path.segments;
            if segments.len() < 2 {
                return None;
            }
            let method = segments.last()?.ident.to_string();
            if method != "new" && method != "default" {
                return None;
            }
            Some(segments[segments.len() - 2].ident.to_string())
        }
        syn::Expr::Reference(reference) => initializer_type(&reference.expr),
        _ => None
    }
}

impl<'ast> Visit<'ast> for FactCollector {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            let segments = &path.path.segments;
            if segments.len() == 1 {
                self.record_call(None, &segments[0].ident.to_string());
            } else if segments.len() >= 2 {
                let owner = segments[segments.len() - 2].ident.to_string();
                let callee = segments[segments.len() - 1].ident.to_string();
                self.record_call(Some(&owner), &callee);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let name = match &local.pat {
            syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
            syn::Pat::Type(typed) => {
                if let syn::Pat::Ident(pattern) = typed.pat.as_ref() {
                    if let Some(type_name) = type_last_segment(&typed.ty) {
                        self.param_types
                            .insert(pattern.ident.to_string(), type_name);
                    }
                    if let Some(element) = element_type(&typed.ty) {
                        self.elem_types.insert(pattern.ident.to_string(), element);
                    }
                    Some(pattern.ident.to_string())
                } else {
                    None
                }
            }
            _ => None
        };
        if let (Some(name), init) = (name.clone(), &local.init)
            && let Some(inner) = init.as_ref().and_then(|init| self.wrapped_of(&init.expr))
        {
            self.local_wraps.insert(name, inner);
        }
        if let (Some(name), init) = (name, &local.init) {
            let constructed = init.as_ref().and_then(|init| {
                initializer_type(&init.expr).or_else(|| self.called_return(&init.expr))
            });
            match constructed {
                Some(type_name) => {
                    self.param_types.insert(name.clone(), type_name);
                    self.fresh.insert(name);
                }
                None => {
                    self.fresh.remove(&name);
                }
            }
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_for_loop(&mut self, loop_expr: &'ast syn::ExprForLoop) {
        self.bind_element(&loop_expr.pat, &loop_expr.expr);
        self.visit_expr(&loop_expr.expr);
        let body = self.flow_scope().block_flow(&loop_expr.body);
        let multiplicity = body_multiplicity(&body, loop_expr.label.as_ref());
        self.repeating(multiplicity, |collector| {
            collector.under(super::output::Guard::For, |collector| {
                collector.visit_block(&loop_expr.body);
            });
        });
    }

    /// A `while` re-evaluates its condition before every iteration, so a
    /// write standing in the condition repeats exactly as one in the
    /// body does. The condition still carries no guard: it is reached
    /// whenever the loop is.
    fn visit_expr_while(&mut self, loop_expr: &'ast syn::ExprWhile) {
        let body = self.flow_scope().block_flow(&loop_expr.body);
        let multiplicity = body_multiplicity(&body, loop_expr.label.as_ref());
        self.repeating(multiplicity, |collector| {
            collector.visit_expr(&loop_expr.cond);
            collector.under(super::output::Guard::While, |collector| {
                collector.visit_block(&loop_expr.body);
            });
        });
    }

    /// A bare `loop` repeats until something leaves it, and its body is
    /// entered unconditionally: the repetition is recorded while the
    /// guard stack stays as it was, so a write inside keeps the
    /// certainty of the position the loop stands in. How many times it
    /// repeats is the body's answer, not the loop keyword's.
    fn visit_expr_loop(&mut self, loop_expr: &'ast syn::ExprLoop) {
        let body = self.flow_scope().block_flow(&loop_expr.body);
        let multiplicity = body_multiplicity(&body, loop_expr.label.as_ref());
        self.repeating(multiplicity, |collector| {
            collector.visit_block(&loop_expr.body);
        });
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.visit_expr(&conditional.cond);
        self.under(super::output::Guard::If, |collector| {
            collector.visit_block(&conditional.then_branch);
        });
        if let Some((_, otherwise)) = &conditional.else_branch {
            self.under(super::output::Guard::Else, |collector| {
                collector.visit_expr(otherwise);
            });
        }
    }

    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        self.visit_expr(&matched.expr);
        let single = matched.arms.len() == 1;
        for arm in &matched.arms {
            let frame = super::output::Guard::MatchArm {
                irrefutable: irrefutable(&arm.pat),
                guarded:     arm_condition(&arm.pat).is_some()
            };
            let binds = single
                && matches!(
                    frame,
                    super::output::Guard::MatchArm {
                        irrefutable: true,
                        guarded:     false
                    }
                );
            if binds {
                self.visit_arm(arm);
            } else {
                self.under(frame, |collector| {
                    collector.visit_arm(arm);
                });
            }
        }
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.under(super::output::Guard::Closure, |collector| {
            syn::visit::visit_expr_closure(collector, closure);
        });
    }

    /// An `async` block builds a future whose body runs where the future
    /// is driven, not where it is written — the same shape a closure
    /// has, and it carries the same frame. Without one, a write in a
    /// future that is never polled was graded as running on every path.
    fn visit_expr_async(&mut self, block: &'ast syn::ExprAsync) {
        self.under(super::output::Guard::Closure, |collector| {
            syn::visit::visit_expr_async(collector, block);
        });
    }

    /// A nested item is order-independent: it is visible to the whole
    /// block around it and is callable from before the line it is
    /// written on. What stands over that line therefore says nothing
    /// about it, so the enclosing divergence and the exits counted
    /// before it are both dropped on the way in and restored on the way
    /// out.
    fn visit_item_fn(&mut self, nested: &'ast syn::ItemFn) {
        let entry_dead = self.dead;
        let entry_exits = self.prior_exits;
        self.dead = false;
        self.prior_exits = 0;
        self.under(super::output::Guard::NestedItem, |collector| {
            syn::visit::visit_item_fn(collector, nested);
        });
        self.dead = entry_dead;
        self.prior_exits = entry_exits;
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let entry_exits = self.prior_exits;
        let entry_dead = self.dead;
        for statement in &block.stmts {
            if gated(statement) {
                self.under(super::output::Guard::CfgGated, |collector| {
                    collector.visit_stmt(statement);
                });
            } else {
                self.visit_stmt(statement);
            }
            let flow = self.flow_scope().statement_flow(statement);
            match flow.completion {
                Completion::Never => self.dead = true,
                Completion::Unknown => self.prior_exits += 1,
                Completion::Always if flow.may_skip() => self.prior_exits += 1,
                Completion::Always => {}
            }
        }
        self.dead = entry_dead;
        self.prior_exits = entry_exits;
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.record_write_target(&assignment.left);
        self.visit_expr(&assignment.right);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.record_write_target(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if let Some(key) = self.field_key(field) {
            self.facts.reads.push(key);
        }
        if let syn::Expr::Path(path) = field.base.as_ref()
            && let Some(ident) = path.path.get_ident()
        {
            let named = matches!(field.member, syn::Member::Named(_));
            self.bump_param(&ident.to_string(), named);
            return;
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if MUTATING_METHODS.contains(&method.as_str())
            && let syn::Expr::Field(field) = call.receiver.as_ref()
            && let Some(key) = self.field_key(field)
        {
            self.record_field_write(field, key);
        }
        if method == "clone" || method == "to_owned" || method == "to_string" {
            self.facts.clones += 1;
        }
        if method == BYTE_WRITE_METHOD {
            self.record_byte_output(call);
        }
        if ITERATOR_ADAPTERS.contains(&method.as_str()) {
            for argument in &call.args {
                if let syn::Expr::Closure(closure) = argument
                    && closure.inputs.len() == 1
                    && let Some(pattern) = closure.inputs.first()
                {
                    self.bind_element(pattern, &call.receiver);
                }
            }
        } else {
            self.bind_wrapped_closure(call);
        }
        if method == knowledge::SERVICE_CALL
            && let Some(inner) = self.wrapped_of(&call.receiver)
        {
            self.record_call(Some(&inner), &method);
        }
        match self.expr_type(&call.receiver) {
            Some(owner) => {
                if let Some(effect) = knowledge::method_effect(&owner, &method)
                    .or_else(|| crate::semantic::std_knowledge::method_effect(&owner, &method))
                {
                    self.facts.effects.push(effect);
                }
                self.record_call(Some(&owner), &method);
            }
            None => self.record_call(Some("?"), &method)
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_struct(&mut self, literal: &'ast syn::ExprStruct) {
        if let Some(segment) = literal.path.segments.last() {
            let name = segment.ident.to_string();
            let constructed = if name == "Self" {
                self.param_types.get("self").cloned().unwrap_or(name)
            } else {
                name
            };
            self.facts.constructs.push(constructed);
        }
        syn::visit::visit_expr_struct(self, literal);
    }

    fn visit_expr_unsafe(&mut self, block: &'ast syn::ExprUnsafe) {
        self.facts.effects.push(Effect::Unsafe);
        syn::visit::visit_expr_unsafe(self, block);
    }

    fn visit_expr_try(&mut self, tried: &'ast syn::ExprTry) {
        if let [enclosing] = self.facts.error_types.clone().as_slice()
            && let Some(key) = self.call_key(&tried.expr)
            && let Some(callee_error) = self.ret(&key).and_then(|r| r.error.clone())
            && callee_error != *enclosing
        {
            self.record_call(Some(enclosing), "from");
        }
        syn::visit::visit_expr_try(self, tried);
    }

    fn visit_expr_await(&mut self, awaited: &'ast syn::ExprAwait) {
        if let Some(future_type) = self.called_return(&awaited.base)
            && self.ret(&format!("{future_type}::poll")).is_some()
        {
            self.record_call(Some(&future_type), "poll");
        }
        syn::visit::visit_expr_await(self, awaited);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.path.segments.len() >= 2 {
            self.record_path_effect(&path.path);
            let first = path.path.segments[0].ident.to_string();
            let last = path.path.segments.last().map(|s| s.ident.to_string());
            if matches!(
                (first.as_str(), last.as_deref()),
                (
                    "Box" | "Vec" | "String" | "Rc" | "Arc",
                    Some("new" | "from" | "with_capacity")
                )
            ) {
                self.facts.allocations += 1;
            }
        } else if let Some(ident) = path.path.get_ident() {
            let name = ident.to_string();
            if self.imports.contains_key(&name) {
                self.record_path_effect(&path.path);
            }
            self.bump_param(&name, false);
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if let Some(segment) = invocation.path.segments.last() {
            let name = segment.ident.to_string();
            if let Some((_, stream, newline)) = OUTPUT_MACROS
                .iter()
                .find(|(macro_name, _, _)| *macro_name == name)
            {
                self.facts.effects.push(Effect::ConsoleIo);
                self.record_output(invocation, *stream, *newline, name == "dbg");
            }
            if let Some((_, newline)) = WRITER_MACROS
                .iter()
                .find(|(macro_name, _)| *macro_name == name)
            {
                self.record_writer_output(invocation, *newline);
            }
            if name == "vec" || name == "format" {
                self.facts.allocations += 1;
            }
            if DIVERGING_MACROS.contains(&name.as_str()) {
                self.facts.flags = self.facts.flags.with_panic();
            }
            if let Some(effect) =
                knowledge::sql_macro_effect(&name, &invocation.tokens.to_string())
            {
                self.facts.effects.push(effect);
            }
            if EXPRESSION_MACROS.contains(&name.as_str()) {
                self.visit_macro_arguments(invocation);
            }
        }
        syn::visit::visit_macro(self, invocation);
    }
}
