// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program does for an observer outside the process.
//!
//! One claim per recorded write site, reachable from an entry point or
//! not: the stream it goes to, the text it puts there, and how certain
//! both are. Certainty has two independent parts that are never merged
//! into a score — whether the owning function is reached from the entry,
//! and whether the site runs on every path through its own function. A
//! site the call graph never reaches keeps its claim and states that,
//! because dropping it would turn a fact about the call graph into an
//! empty report about the recognizer. The rendered sentence degrades
//! field by field: a weakened certainty names every construct that
//! weakened it, a partially proven payload prints its proven text with
//! the holes left as holes, and an unidentifiable stream says so instead
//! of guessing.
//!
//! Two properties of the standard library, not of the analyzed program,
//! are attached to every claim about a standard stream: print output can
//! be diverted by the library's capture hook, and a failed write panics
//! rather than writing. A claim is therefore about an attempted write.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _
};

use super::{
    behavior::{
        FunctionFacts, Grade, Guard, Multiplicity, Newline, OUTPUT_SINK_SET, OpaqueReason,
        OutputWrite, Payload, Segment, SiteGuard, Stream, behavior
    },
    calls::{CallGraph, call_graph, resolution_context},
    entry::{EntryKind, EntryPoint, entry_points}
};
use crate::source::Source;

/// One statement about what the program writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputClaim {
    /// Function holding the write site.
    pub function:  String,
    /// File the site lives in.
    pub file:      String,
    /// The recorded write.
    pub write:     OutputWrite,
    /// How certainly the owning function is reached from the entry.
    ///
    /// [`Grade::Unreached`] is a claim of its own — the call graph found
    /// no path from this entry to the site — and is stated rather than
    /// dropped, so an empty claim list means the recognizer found no
    /// write at all and nothing else.
    pub reach:     Grade,
    /// How certainly the site runs, both parts composed.
    pub execution: Grade,
    /// Version of the recognizer list that produced this claim.
    pub sink_set:  u32
}

/// Everything one entry point is observed to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeaning {
    /// Path of the binary root declaring the entry.
    pub file:   String,
    /// How the entry is declared.
    pub kind:   EntryKind,
    /// Claims in the order their sites were recorded, one per recorded
    /// write, including the writes no path from this entry reaches —
    /// those carry [`Grade::Unreached`] rather than being dropped.
    pub claims: Vec<OutputClaim>
}

/// Derives what every entry point of `sources` does.
#[must_use]
pub fn program_meaning(sources: &[Source]) -> Vec<EntryMeaning> {
    let facts = behavior(sources);
    let context = resolution_context(sources);
    let graph = call_graph(&facts, &context);
    let qualifiers = qualifiers_by_fact(sources, &facts);
    entry_points(sources, &facts)
        .into_iter()
        .map(|entry| meaning_of(&entry, &facts, &graph, &qualifiers, sources))
        .collect()
}

/// Builds the claims of one entry point.
///
/// Every recorded write becomes a claim. A write the call graph does not
/// reach from this entry is stated as unreached, not dropped: dropping
/// it would leave an empty list that reads as "the recognizer found
/// nothing", which is a different fact and, here, a false one.
fn meaning_of(
    entry: &EntryPoint,
    facts: &[FunctionFacts],
    graph: &CallGraph,
    qualifiers: &[BTreeSet<String>],
    sources: &[Source]
) -> EntryMeaning {
    let reach = reachability(facts, graph, qualifiers, entry.fact);
    let mut claims = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        let file = sources.get(fact.source).map_or_else(String::new, |source| {
            source.path().to_string_lossy().replace('\\', "/")
        });
        for write in &fact.output_writes {
            claims.push(OutputClaim {
                function:  fact.function.clone(),
                file:      file.clone(),
                execution: reach[index].min(write.guard.local_grade()),
                reach:     reach[index],
                write:     write.clone(),
                sink_set:  OUTPUT_SINK_SET
            });
        }
    }
    EntryMeaning {
        file: entry.file.clone(),
        kind: entry.kind,
        claims
    }
}

/// Grades how certainly every function is reached from `entry`.
///
/// The entry itself is certain. A callee is graded by the weaker of its
/// caller's grade and the grade of the edge between them, and keeps the
/// strongest grade any path gives it. An edge is possible when the
/// dispatch behind it names several implementors, otherwise it is graded
/// by the call sites that provably bind to that callee. A function no
/// path grades stays [`Grade::Unreached`], which is a statement about
/// the call graph and not about the function.
fn reachability(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    qualifiers: &[BTreeSet<String>],
    entry: usize
) -> Vec<Grade> {
    if entry >= facts.len() {
        return vec![Grade::Unreached; facts.len()];
    }
    let bound = bound_sites(facts, qualifiers);
    let mut grades: Vec<Option<Grade>> = vec![None; facts.len()];
    grades[entry] = Some(Grade::Certain);
    let mut changed = true;
    while changed {
        changed = false;
        for caller in 0..facts.len() {
            let Some(reached) = grades[caller] else {
                continue;
            };
            for &callee in graph.callees(caller) {
                let edge = if graph.possibilistic(caller, callee) {
                    Grade::Possible
                } else {
                    bound[caller]
                        .get(&callee)
                        .copied()
                        .unwrap_or(Grade::Possible)
                };
                let candidate = reached.min(edge);
                if grades[callee].is_none_or(|known| candidate > known) {
                    grades[callee] = Some(candidate);
                    changed = true;
                }
            }
        }
    }
    grades
        .into_iter()
        .map(|grade| grade.unwrap_or(Grade::Unreached))
        .collect()
}

/// Grades every caller's edges by the call sites that provably bind to
/// them: caller index to callee index to the strongest grade among the
/// binding sites.
///
/// A callee reached only through sites that can never run is itself
/// dead; a callee no site binds to is absent from the map, and its edge
/// keeps the weakest honest grade — the graph proved the call, the
/// sites did not prove when it runs.
fn bound_sites(
    facts: &[FunctionFacts],
    qualifiers: &[BTreeSet<String>]
) -> Vec<BTreeMap<usize, Grade>> {
    let sole = sole_by_name(facts);
    facts
        .iter()
        .map(|fact| {
            let mut grades: BTreeMap<usize, Grade> = BTreeMap::new();
            for site in &fact.call_sites {
                let Some(callee) = bound_callee(&sole, qualifiers, &site.candidate) else {
                    continue;
                };
                let grade = site.guard.local_grade();
                grades
                    .entry(callee)
                    .and_modify(|known| *known = (*known).max(grade))
                    .or_insert(grade);
            }
            grades
        })
        .collect()
}

/// Index of every function whose name no other function shares, so a
/// candidate spelling one of these names cannot mean a second function.
fn sole_by_name(facts: &[FunctionFacts]) -> BTreeMap<&str, usize> {
    let mut seen: BTreeMap<&str, Option<usize>> = BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        seen.entry(fact.function.as_str())
            .and_modify(|slot| *slot = None)
            .or_insert(Some(index));
    }
    seen.into_iter()
        .filter_map(|(name, index)| index.map(|index| (name, index)))
        .collect()
}

/// The one function a recorded call candidate can name, or `None` when
/// the spelling admits another.
///
/// Two bindings are provable: the candidate is a function's own name and
/// no other function carries it, or the candidate is `module::name`,
/// exactly one function is called `name`, and `module` is the very scope
/// that declares it — the qualifier must own the callee, not merely
/// exist somewhere in the workspace. Any workspace module would do
/// otherwise, and `elsewhere::name` would certify a function that module
/// does not hold. A candidate whose receiver type is unknown
/// (`?::method`) names a method rather than a function and binds to
/// nothing — matching on the method name alone would bind whichever type
/// happened to declare it.
fn bound_callee(
    sole: &BTreeMap<&str, usize>,
    qualifiers: &[BTreeSet<String>],
    candidate: &str
) -> Option<usize> {
    if candidate.starts_with("?::") {
        return None;
    }
    if let Some(index) = sole.get(candidate) {
        return Some(*index);
    }
    let (qualifier, bare) = candidate.rsplit_once("::")?;
    let index = *sole.get(bare)?;
    qualifiers.get(index)?.contains(qualifier).then_some(index)
}

/// Names that may qualify each function in a call, by fact index.
///
/// A free function answers to the scope that declares it and to nothing
/// else: the innermost inline module around it, or — at a file's top
/// level — the module the file itself is, which is its stem, its
/// directory for a `mod.rs`, and the crate for a crate root. A function
/// no scope could be read for keeps an empty set, so a qualified
/// candidate never binds to it by proximity of spelling alone.
fn qualifiers_by_fact(sources: &[Source], facts: &[FunctionFacts]) -> Vec<BTreeSet<String>> {
    let mut positions: BTreeMap<(usize, u32, u32), usize> = BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        positions.insert((fact.source, fact.range.start, fact.range.end), index);
    }
    let mut qualifiers = vec![BTreeSet::new(); facts.len()];
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let scope = file_scope(&source.path().to_string_lossy());
        scope_items(&file.items, &scope, index, &positions, &mut qualifiers);
    }
    qualifiers
}

/// Records the scope of every function declared directly in `items`,
/// descending into inline modules, which replace the scope for what they
/// hold.
fn scope_items(
    items: &[syn::Item],
    scope: &BTreeSet<String>,
    source: usize,
    positions: &BTreeMap<(usize, u32, u32), usize>,
    qualifiers: &mut [BTreeSet<String>]
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                let span = function.sig.ident.span().byte_range();
                let start = u32::try_from(span.start).unwrap_or(u32::MAX);
                let end = u32::try_from(span.end).unwrap_or(u32::MAX);
                if let Some(index) = positions.get(&(source, start, end))
                    && let Some(slot) = qualifiers.get_mut(*index)
                {
                    slot.clone_from(scope);
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    let inner: BTreeSet<String> =
                        std::iter::once(module.ident.to_string()).collect();
                    scope_items(nested, &inner, source, positions, qualifiers);
                }
            }
            _ => {}
        }
    }
}

/// The module names a file's own top level answers to.
fn file_scope(path: &str) -> BTreeSet<String> {
    let normalized = path.replace('\\', "/");
    let components: Vec<&str> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let mut names = BTreeSet::new();
    let Some((last, directories)) = components.split_last() else {
        return names;
    };
    match last.strip_suffix(".rs").unwrap_or(last) {
        "mod" => {
            if let Some(parent) = directories.last() {
                names.insert((*parent).to_owned());
            }
        }
        "lib" | "main" => {
            names.insert("crate".to_owned());
            if let Some(krate) = crate_name(directories) {
                names.insert(krate);
            }
        }
        stem => {
            names.insert(stem.to_owned());
        }
    }
    names
}

/// The crate a source directory belongs to, read from the `<crate>/src/`
/// layout with `-` normalized to `_` as the module system spells it.
fn crate_name(directories: &[&str]) -> Option<String> {
    let at = directories.iter().position(|part| *part == "src")?;
    let owner = at.checked_sub(1).and_then(|index| directories.get(index))?;
    Some(owner.replace('-', "_"))
}

impl EntryMeaning {
    /// Renders every claim as one sentence in the order the claims were
    /// recorded, each degraded exactly as far as its weakest field
    /// allows.
    ///
    /// An empty claim list means one thing only: the recognizer set of
    /// this build matched no write anywhere in the sources. It never
    /// means a write was found and set aside.
    #[must_use]
    pub fn sentences(&self) -> Vec<String> {
        if self.claims.is_empty() {
            return vec![format!(
                "no recognized write to a standard stream (recognizer set v{OUTPUT_SINK_SET}); \
                 this states the recognizer found none, not that the program writes nothing"
            )];
        }
        self.claims.iter().map(OutputClaim::sentence).collect()
    }
}

impl OutputClaim {
    /// What stands between this claim and certainty, or `None` when
    /// nothing does.
    #[must_use]
    pub fn weakening(&self) -> Option<String> {
        weakening_of(self)
    }

    /// What the sentence still has to name after its opening words.
    ///
    /// The opening of an unreached claim already states the call chain,
    /// so repeating it would say the same thing twice; everything else
    /// the claim carries is named at every grade below certainty,
    /// because a claim that is dead or unreached has a cause exactly as
    /// a possible one does.
    fn stated_weakening(&self) -> Option<String> {
        let reasons = match self.execution {
            Grade::Certain => Vec::new(),
            Grade::Unreached => site_reasons(&self.write.guard),
            Grade::Possible | Grade::Dead => chain_reason(self.reach)
                .into_iter()
                .map(ToOwned::to_owned)
                .chain(site_reasons(&self.write.guard))
                .collect()
        };
        (!reasons.is_empty()).then(|| reasons.join(" and "))
    }

    /// The text this claim proves, rendered without ever standing in a
    /// value it cannot see: holes keep their names, an unreadable
    /// payload states its reason.
    #[must_use]
    pub fn payload_text(&self) -> String {
        match &self.write.payload {
            Payload::Empty => "<nothing>".to_owned(),
            Payload::Literal(text) => format!("{:?}", with_terminator(text, self.write.newline)),
            Payload::Template(segments) => {
                let shape: String = segments
                    .iter()
                    .map(|segment| match segment {
                        Segment::Text(text) => text.clone(),
                        Segment::Hole(hole) => format!("<{}>", hole_name(hole))
                    })
                    .collect();
                format!("{:?}", with_terminator(&shape, self.write.newline))
            }
            Payload::Opaque(reason) => format!("<unreadable: {}>", reason_text(*reason))
        }
    }

    /// The sentence this claim licenses.
    #[must_use]
    pub fn sentence(&self) -> String {
        let opening = match self.execution {
            Grade::Certain => "the process unconditionally writes",
            Grade::Possible => "the process may write",
            Grade::Dead => "the process contains a write that can never run:",
            Grade::Unreached => "no path from the entry point reaches a write of"
        };
        let destination = match self.write.stream {
            Stream::Stdout => "its standard output stream",
            Stream::Stderr => "its standard error stream",
            Stream::CallerSupplied => "a stream the caller supplies",
            Stream::Unknown => "a stream this analysis cannot identify"
        };
        let mut sentence = format!(
            "{opening} {} to {destination}",
            describe(&self.write.payload, self.write.newline)
        );
        if self.write.multiplicity == Multiplicity::Unbounded {
            sentence.push_str(", once per iteration of the loop around it");
        }
        if let Some(reason) = self.stated_weakening() {
            let _ = write!(sentence, ", weakened by {reason}");
        }
        let _ = write!(
            sentence,
            " [{} in `{}`, attempted: the library can divert or fail the write]",
            self.file, self.function
        );
        sentence
    }
}

/// Names everything that stands between a claim and certainty.
///
/// Reach and local execution are two independent weakenings and a claim
/// can carry both; naming only the first found would hide the other and
/// report a doubly weakened claim as singly weakened. The order is
/// outside-in: the call chain first, then the constructs standing over
/// the site inside its own function.
fn weakening_of(claim: &OutputClaim) -> Option<String> {
    if claim.execution == Grade::Certain {
        return None;
    }
    let reasons: Vec<String> = chain_reason(claim.reach)
        .into_iter()
        .map(ToOwned::to_owned)
        .chain(site_reasons(&claim.write.guard))
        .collect();
    (!reasons.is_empty()).then(|| reasons.join(" and "))
}

/// What the call chain does to a claim, or `None` when it does nothing.
const fn chain_reason(reach: Grade) -> Option<&'static str> {
    match reach {
        Grade::Certain => None,
        Grade::Possible => Some("a call chain that does not certainly reach it"),
        Grade::Unreached => Some("a call chain that never reaches it"),
        Grade::Dead => Some("a call chain whose every call to it can never run")
    }
}

/// Every construct standing over the site inside its own function, each
/// named once however many times it stands there.
fn site_reasons(guard: &SiteGuard) -> Vec<String> {
    let named: BTreeSet<&'static str> = guard
        .guards
        .iter()
        .map(|guard| match guard {
            Guard::If | Guard::Else => "a branch",
            Guard::MatchArm {
                ..
            } => "a match arm",
            Guard::For | Guard::While => "a loop that may not run",
            Guard::Closure => "a closure that runs only where it is called",
            Guard::NestedItem => "a nested item that runs only when called",
            Guard::CfgGated => "a configuration gate this analysis does not resolve"
        })
        .collect();
    let mut reasons: Vec<String> = named.into_iter().map(ToOwned::to_owned).collect();
    if guard.dead {
        reasons.push("a divergence before it that always leaves the function".to_owned());
    }
    if guard.prior_exits > 0 {
        reasons.push("an exit that may leave the function first".to_owned());
    }
    reasons
}

/// Appends the line terminator a construct adds, so the rendered text
/// is what actually reaches the stream.
fn with_terminator(text: &str, newline: Newline) -> String {
    match newline {
        Newline::Appended => format!("{text}\n"),
        Newline::None => text.to_owned()
    }
}

/// Why a payload could not be read, in words.
const fn reason_text(reason: OpaqueReason) -> &'static str {
    match reason {
        OpaqueReason::RuntimeValue => "the written value is computed at runtime",
        OpaqueReason::NonLiteralFormat => "the format string is not a literal",
        OpaqueReason::Unparsed => "the arguments did not parse",
        OpaqueReason::DebugMacro => "the debugging macro embeds its own source position"
    }
}

/// Renders a payload without ever claiming an unproven value.
fn describe(payload: &Payload, newline: Newline) -> String {
    let terminator = match newline {
        Newline::Appended => " followed by a line terminator",
        Newline::None => ""
    };
    match payload {
        Payload::Empty => "nothing".to_owned(),
        Payload::Literal(text) => format!("the string `{text}`{terminator}"),
        Payload::Template(segments) => {
            let shape: String = segments
                .iter()
                .map(|segment| match segment {
                    Segment::Text(text) => text.clone(),
                    Segment::Hole(hole) => format!("<hole:{}>", hole_name(hole))
                })
                .collect();
            format!(
                "text of the form `{shape}`{terminator}, where each hole is a runtime value this \
                 analysis does not determine"
            )
        }
        Payload::Opaque(reason) => format!(
            "a value this analysis cannot read ({})",
            match reason {
                OpaqueReason::RuntimeValue => "the written value is computed at runtime",
                OpaqueReason::NonLiteralFormat => "the format string is not a literal",
                OpaqueReason::Unparsed => "the arguments did not parse",
                OpaqueReason::DebugMacro => "the debugging macro embeds its own source position"
            }
        )
    }
}

/// Names one hole by whatever the source gives it.
fn hole_name(hole: &super::behavior::Hole) -> String {
    match &hole.source {
        super::behavior::HoleSource::Named(name) => name.clone(),
        super::behavior::HoleSource::Positional(index)
        | super::behavior::HoleSource::Indexed(index) => hole
            .expr
            .clone()
            .unwrap_or_else(|| format!("argument {index}"))
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

    use super::program_meaning;
    use crate::source::Source;

    fn sentences(body: &str) -> Vec<String> {
        sentences_of(&[("src/main.rs", body)])
    }

    /// Every sentence of the first entry point of a whole file set, so a
    /// claim about cross-file qualification is stated on the layout it
    /// depends on.
    fn sentences_of(files: &[(&str, &str)]) -> Vec<String> {
        let sources: Vec<Source> = files
            .iter()
            .map(|(path, body)| Source::new(PathBuf::from(path), (*body).to_owned()))
            .collect();
        program_meaning(&sources)
            .first()
            .map(super::EntryMeaning::sentences)
            .unwrap_or_default()
    }

    #[test]
    fn the_smallest_program_yields_the_strongest_sentence() {
        let said = sentences("fn main() {\n    println!(\"Hello, world!\");\n}");
        assert_eq!(said.len(), 1);
        assert!(
            said[0].starts_with(
                "the process unconditionally writes the string `Hello, world!` followed by a \
                 line terminator to its standard output stream"
            ),
            "{}",
            said[0]
        );
    }

    #[test]
    fn a_branch_degrades_the_sentence_and_names_what_weakened_it() {
        let said = sentences("fn main() {\n    if flag { println!(\"Hello\"); }\n}");
        assert!(said[0].contains("may write"), "{}", said[0]);
        assert!(said[0].contains("weakened by a branch"), "{}", said[0]);
    }

    #[test]
    fn a_runtime_value_is_never_rendered_as_text() {
        let said = sentences("fn main() {\n    println!(\"hi {name}\");\n}");
        assert!(
            said[0].contains("text of the form `hi <hole:name>`"),
            "{}",
            said[0]
        );
        assert!(
            said[0].contains("this analysis does not determine"),
            "{}",
            said[0]
        );
    }

    #[test]
    fn a_write_behind_a_call_keeps_its_certainty_across_the_boundary() {
        let said =
            sentences("fn greet() { println!(\"Hello, world!\"); }\nfn main() {\n    greet();\n}");
        assert!(said[0].contains("unconditionally writes"), "{}", said[0]);
        assert!(said[0].contains("in `greet`"), "{}", said[0]);
    }

    #[test]
    fn a_program_without_a_recognized_write_says_so_without_claiming_silence() {
        let said = sentences("fn main() {\n    let value = 1 + 1;\n}");
        assert_eq!(said.len(), 1);
        assert!(said[0].starts_with("no recognized write"), "{}", said[0]);
        assert!(said[0].contains("not that the program writes nothing"));
    }

    #[test]
    fn a_loop_reports_repetition_rather_than_a_single_write() {
        let said = sentences("fn main() {\n    for _ in 0..3 { println!(\"tick\"); }\n}");
        assert!(said[0].contains("once per iteration"), "{}", said[0]);
    }

    /// The one sentence mentioning `text`, so a test names the claim it
    /// is about instead of an index into a list that may grow.
    fn about(said: &[String], text: &str) -> String {
        let mut matching = said.iter().filter(|sentence| sentence.contains(text));
        let found = matching
            .next()
            .unwrap_or_else(|| panic!("no sentence about `{text}` in {said:#?}"));
        assert!(
            matching.next().is_none(),
            "several sentences about `{text}` in {said:#?}"
        );
        found.clone()
    }

    #[test]
    fn a_call_on_one_type_never_certifies_the_same_method_name_on_another() {
        let said = sentences(
            "struct Ready;\nstruct Lazy;\nimpl Ready {\n    fn go() { println!(\"ready\"); \
             }\n}\nimpl Lazy {\n    fn go() { println!(\"lazy\"); }\n}\nfn main() {\n    \
             Ready::go();\n    if flag { Lazy::go(); }\n}"
        );
        let ready = about(&said, "ready");
        let lazy = about(&said, "lazy");
        assert!(ready.contains("unconditionally writes"), "{ready}");
        assert!(
            lazy.contains("may write"),
            "the only call to `Lazy::go` stands under a branch: {lazy}"
        );
    }

    #[test]
    fn an_unknown_receiver_never_certifies_the_only_method_of_that_name() {
        let said = sentences(
            "struct Lazy;\nimpl Lazy {\n    fn go(&self) { println!(\"lazy\"); }\n}\nfn main() \
             {\n    let value = elsewhere();\n    value.go();\n}"
        );
        let lazy = about(&said, "lazy");
        assert!(
            lazy.contains("may write"),
            "the receiver's type is unknown, so the callee is not proven: {lazy}"
        );
    }

    #[test]
    fn a_function_called_only_from_a_dead_site_writes_nothing() {
        let said = sentences(
            "fn shout() { println!(\"never\"); }\nfn main() {\n    return;\n    shout();\n}"
        );
        let never = about(&said, "never");
        assert!(never.contains("can never run"), "{never}");
    }

    #[test]
    fn a_claim_weakened_twice_names_both_weakeners() {
        let said = sentences(
            "fn shout() {\n    if inner { println!(\"hi\"); }\n}\nfn main() {\n    if outer { \
             shout(); }\n}"
        );
        let claim = about(&said, "hi");
        assert!(
            claim.contains("a call chain that does not certainly reach it"),
            "{claim}"
        );
        assert!(claim.contains("a branch"), "{claim}");
    }

    #[test]
    fn a_qualifier_naming_another_module_never_certifies_the_call() {
        let said = sentences(
            "mod alpha {\n    pub fn go() { println!(\"alpha\"); }\n}\nmod beta {\n    pub fn \
             stub() { let value = 1; }\n}\nfn main() {\n    beta::go();\n}"
        );
        let alpha = about(&said, "alpha");
        assert!(
            !alpha.contains("unconditionally writes"),
            "`beta::go` is not `alpha::go`; the qualifier does not own the callee: {alpha}"
        );
    }

    #[test]
    fn the_module_that_declares_the_function_still_certifies_the_call() {
        let said = sentences(
            "mod alpha {\n    pub fn go() { println!(\"alpha\"); }\n}\nfn main() {\n    \
             alpha::go();\n}"
        );
        let alpha = about(&said, "alpha");
        assert!(
            alpha.contains("unconditionally writes"),
            "the qualifier names the very module declaring the callee: {alpha}"
        );
    }

    #[test]
    fn a_file_module_certifies_its_own_function_and_no_other_files() {
        let said = sentences_of(&[
            (
                "src/main.rs",
                "mod alpha;\nmod beta;\nfn main() {\n    alpha::go();\n}"
            ),
            ("src/alpha.rs", "pub fn go() { println!(\"alpha\"); }"),
            ("src/beta.rs", "pub fn stub() { let value = 1; }")
        ]);
        let alpha = about(&said, "alpha");
        assert!(
            alpha.contains("unconditionally writes"),
            "the file is the module the qualifier names: {alpha}"
        );
        let other = sentences_of(&[
            (
                "src/main.rs",
                "mod alpha;\nmod beta;\nfn main() {\n    beta::go();\n}"
            ),
            ("src/alpha.rs", "pub fn go() { println!(\"alpha\"); }"),
            ("src/beta.rs", "pub fn stub() { let value = 1; }")
        ]);
        let claimed = about(&other, "alpha");
        assert!(
            !claimed.contains("unconditionally writes"),
            "`beta` does not declare `go`: {claimed}"
        );
    }

    #[test]
    fn a_dead_claim_names_what_killed_it() {
        let said = sentences(
            "fn shout() { println!(\"never\"); }\nfn main() {\n    return;\n    shout();\n}"
        );
        let never = about(&said, "never");
        assert!(
            never.contains("a call chain whose every call to it can never run"),
            "a claim that can never run states its cause like every other: {never}"
        );
    }

    #[test]
    fn an_unreached_claim_names_the_site_without_restating_the_chain() {
        let said =
            sentences("fn orphan() {\n    if flag { println!(\"orphan\"); }\n}\nfn main() {}");
        let orphan = about(&said, "orphan");
        assert!(orphan.contains("weakened by a branch"), "{orphan}");
        assert!(
            !orphan.contains("a call chain that never reaches it"),
            "the opening already states the chain; saying it twice states nothing new: {orphan}"
        );
    }

    #[test]
    fn a_write_the_call_graph_never_reaches_is_stated_rather_than_dropped() {
        let said = sentences("fn orphan() { println!(\"orphan\"); }\nfn main() {}");
        assert_eq!(said.len(), 1, "{said:#?}");
        assert!(
            said[0].contains("no path from the entry point reaches"),
            "{}",
            said[0]
        );
        assert!(
            !said[0].starts_with("no recognized write"),
            "a recognized write was found; the recognizer set is not what emptied the report"
        );
    }
}
