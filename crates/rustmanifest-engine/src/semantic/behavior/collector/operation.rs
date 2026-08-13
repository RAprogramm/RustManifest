// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one operation over values.
//!
//! Four shapes compute in the language itself — a binary operator, a
//! negation, an element taken at a position, a written cast — and one
//! comes from the library, where a method names both the operation and
//! what it does with the failure that operation can have.
//!
//! An operator is syntax and is read as syntax: the "library semantics
//! live as data" rule is about the library, and a table of the language's
//! own operators would restate the grammar. The library's spellings do
//! live in a table, and a method absent from it is not read as an
//! operation at all — a namesake of the program's own would otherwise be
//! credited with the library's behaviour.

mod conversion;

use super::FactCollector;
use crate::semantic::{
    behavior::{
        operation::{
            Arithmetic, Comparison, Conversion, Guarding, Junction, Operation, OperationSite
        },
        output::Multiplicity,
        value::{Disposal, Literal, Provenance}
    },
    std_knowledge::operation_method
};

impl FactCollector {
    /// Records one operation, answering with its position in the channel.
    ///
    /// The record becomes the origin of the value the construct produces,
    /// displacing the call that performs it: a conversion is a truer
    /// answer to "where did this value come from" than the method whose
    /// name happens to perform it. A read of the boundary is truer still
    /// and is left in place.
    fn push_operation(
        &mut self,
        span: &std::ops::Range<usize>,
        operation: Operation,
        operands: Vec<Provenance>,
        guarding: Guarding
    ) -> usize {
        let index = self.facts.operations.len();
        let site = rustmanifest_schema::Range {
            start: u32::try_from(span.start).unwrap_or(u32::MAX),
            end:   u32::try_from(span.end).unwrap_or(u32::MAX)
        };
        self.facts.operations.push(OperationSite {
            operation,
            operands,
            produced: None,
            guarding,
            disposal: Disposal::NotIdentified,
            site,
            guard: self.site_guard(),
            conditions: self.site_conditions(),
            multiplicity: Multiplicity::Once
        });
        let named = Provenance::Computed(u32::try_from(index).unwrap_or(u32::MAX));
        match self.produced.get(&(site.start, site.end)) {
            Some(Provenance::Read(_)) => {}
            _ => {
                self.produced.insert((site.start, site.end), named);
            }
        }
        index
    }

    /// Records the operation a binary operator performs, where it is one
    /// this channel recognizes.
    pub(super) fn record_binary_operation(&mut self, binary: &syn::ExprBinary) {
        use syn::spanned::Spanned;

        let Some(operation) = binary_operation(binary.op) else {
            return;
        };
        let operands = vec![self.origin_of(&binary.left), self.origin_of(&binary.right)];
        self.push_operation(
            &binary.span().byte_range(),
            operation,
            operands,
            Guarding::Unchecked
        );
    }

    /// Records the operation a unary operator performs.
    pub(super) fn record_unary_operation(&mut self, unary: &syn::ExprUnary) {
        use syn::spanned::Spanned;

        let performed = match unary.op {
            syn::UnOp::Neg(_) => Arithmetic::Negate,
            syn::UnOp::Not(_) => Arithmetic::Complement,
            _ => return
        };
        let operand = self.origin_of(&unary.expr);
        self.push_operation(
            &unary.span().byte_range(),
            Operation::Arithmetic(performed),
            vec![operand],
            Guarding::Unchecked
        );
    }

    /// Records the element a sequence is indexed for.
    pub(super) fn record_selection(&mut self, indexed: &syn::ExprIndex) {
        use syn::spanned::Spanned;

        let operands = vec![
            self.origin_of(&indexed.expr),
            self.origin_of(&indexed.index),
        ];
        self.push_operation(
            &indexed.span().byte_range(),
            Operation::Selection,
            operands,
            Guarding::Unchecked
        );
    }

    /// Records the element a loop takes out of the value it iterates,
    /// and lets the name the loop binds stand for that element.
    ///
    /// Each iteration's value is one element of the iterated value, which
    /// is the same fact an index at a written position records, so it is
    /// recorded as the same operation. The name is only bound where the
    /// body writes it once: a loop that reassigns its own binding stands
    /// for no single value, and the refusal that says so is the honest
    /// answer.
    pub(super) fn record_iterated_element(
        &mut self,
        pattern: &syn::Pat,
        iterable: &syn::Expr,
        span: &std::ops::Range<usize>
    ) {
        let named = super::naming::binding_name(pattern);
        let takes_apart = matches!(pattern, syn::Pat::Tuple(_));
        if !takes_apart && !named.as_ref().is_some_and(|name| self.sole.contains(name)) {
            return;
        }
        let operands = vec![self.origin_of(iterable)];
        let index = self.push_operation(span, Operation::Selection, operands, Guarding::Unchecked);
        let element = Provenance::Computed(u32::try_from(index).unwrap_or(u32::MAX));
        if self.record_tuple_parts(pattern, &element, span) {
            return;
        }
        if let Some(name) = named {
            self.origins.insert(name, element);
        }
    }

    /// Records every closure input written beside an adapter as one
    /// element of what the adapter runs over.
    pub(super) fn record_adapter_elements(
        &mut self,
        call: &syn::ExprMethodCall,
        span: &std::ops::Range<usize>
    ) {
        for argument in &call.args {
            let syn::Expr::Closure(closure) = argument else {
                continue;
            };
            let [only] = closure.inputs.iter().collect::<Vec<_>>()[..] else {
                continue;
            };
            self.record_iterated_element(only, &call.receiver, span);
        }
    }

    /// The failure half of a value, recorded as the selection that takes
    /// it: position one of a fallible answer, the same position a
    /// pattern writing `Err` takes.
    pub(super) fn failure_half(
        &mut self,
        taken: &Provenance,
        span: &std::ops::Range<usize>
    ) -> Provenance {
        let operands = vec![
            taken.clone(),
            Provenance::Written(Literal::Integer("1".to_owned())),
        ];
        let index = self.push_operation(span, Operation::Selection, operands, Guarding::Unchecked);
        Provenance::Computed(u32::try_from(index).unwrap_or(u32::MAX))
    }

    /// Records each part a pattern takes out of one value, and lets the
    /// name bound at each position stand for that part.
    ///
    /// A pattern that is a plain name takes nothing apart: it binds the
    /// whole value it was matched against, and that value is what the
    /// name stands for.
    ///
    /// A part that is itself a pattern is taken apart in turn, against
    /// the part it stands for: `(Some(one), Some(two))` names position
    /// zero of the pair, then position zero of what that carries, and
    /// each step is the selection the source wrote.
    ///
    /// A name a nearer reading already answered is left alone: the
    /// outcome reading, which knows the value a fallible call handed
    /// back, states it without the unwrapping step, and restating it as a
    /// selection would put a step between a value and itself.
    ///
    /// Which part a name holds is written in the pattern itself: the name
    /// at position `k` holds what stands at position `k` of the value
    /// taken apart — an element of a tuple, or the payload a variant
    /// carries at that position — and the language guarantees it. That is the
    /// same fact an index at a written position records, so it is recorded
    /// as the same operation over the same two operands — the value taken
    /// apart, and the position taken from it.
    pub(super) fn record_tuple_parts(
        &mut self,
        pattern: &syn::Pat,
        taken: &Provenance,
        span: &std::ops::Range<usize>
    ) -> bool {
        let pattern = through_wrappers(pattern);
        if let syn::Pat::Ident(bound) = pattern
            && bound.subpat.is_none()
        {
            let name = bound.ident.to_string();
            if self.sole.contains(&name) && !self.origins.contains_key(&name) {
                self.origins.insert(name, taken.clone());
            }
            return true;
        }
        if let syn::Pat::Struct(written) = pattern {
            return self.record_named_parts(written);
        }
        let parts: Vec<&syn::Pat> = match pattern {
            syn::Pat::Tuple(tuple) => tuple.elems.iter().collect(),
            syn::Pat::TupleStruct(variant) if names_the_failure_half(variant) => {
                self.type_single_payload(variant);
                for part in &variant.elems {
                    let Some(name) = super::naming::binding_name(part) else {
                        continue;
                    };
                    if !self.sole.contains(&name) || self.origins.contains_key(&name) {
                        continue;
                    }
                    let operands = vec![
                        taken.clone(),
                        Provenance::Written(Literal::Integer("1".to_owned())),
                    ];
                    let index = self.push_operation(
                        span,
                        Operation::Selection,
                        operands,
                        Guarding::Unchecked
                    );
                    self.origins.insert(
                        name,
                        Provenance::Computed(u32::try_from(index).unwrap_or(u32::MAX))
                    );
                }
                return true;
            }
            syn::Pat::TupleStruct(variant) => {
                self.type_single_payload(variant);
                variant.elems.iter().collect()
            }
            _ => return false
        };
        self.type_pair_parts(pattern, taken);
        for (position, part) in parts.into_iter().enumerate() {
            let Some(name) = super::naming::binding_name(part) else {
                if !matches!(
                    part,
                    syn::Pat::Tuple(_) | syn::Pat::TupleStruct(_) | syn::Pat::Struct(_)
                ) {
                    continue;
                }
                let operands = vec![
                    taken.clone(),
                    Provenance::Written(Literal::Integer(position.to_string())),
                ];
                let index =
                    self.push_operation(span, Operation::Selection, operands, Guarding::Unchecked);
                let inner = Provenance::Computed(u32::try_from(index).unwrap_or(u32::MAX));
                self.record_tuple_parts(part, &inner, span);
                continue;
            };
            if !self.sole.contains(&name) || self.origins.contains_key(&name) {
                continue;
            }
            let operands = vec![
                taken.clone(),
                Provenance::Written(Literal::Integer(position.to_string())),
            ];
            let index =
                self.push_operation(span, Operation::Selection, operands, Guarding::Unchecked);
            self.origins.insert(
                name,
                Provenance::Computed(u32::try_from(index).unwrap_or(u32::MAX))
            );
        }
        true
    }

    /// Types the two names a pair pattern binds, where the value taken
    /// apart is a map the tables know.
    ///
    /// Iterating a map hands out a key and a value in that order, which
    /// the library fixes and the pattern positions name. Without this
    /// the value of a map iteration types to nothing, and every field
    /// read on it refuses.
    fn type_pair_parts(&mut self, pattern: &syn::Pat, taken: &Provenance) {
        let syn::Pat::Tuple(tuple) = pattern else {
            return;
        };
        let [first, second] = tuple.elems.iter().collect::<Vec<_>>()[..] else {
            return;
        };
        let Provenance::Computed(index) = taken else {
            return;
        };
        let Some(site) = usize::try_from(*index)
            .ok()
            .and_then(|at| self.facts.operations.get(at))
        else {
            return;
        };
        let [over, ..] = site.operands.as_slice() else {
            return;
        };
        let Provenance::Field(_) = over else {
            return;
        };
        self.bind_pattern(first);
        self.bind_pattern(second);
    }

    /// Types the one name a variant pattern binds, where the enum
    /// declares what that variant carries.
    ///
    /// The type is written in the declaration and nowhere near the
    /// position reading it, so without this the name types to nothing and
    /// every field read on it refuses. The qualified spelling is asked
    /// first: a bare variant name two enums both declare answers for
    /// neither.
    fn type_single_payload(&mut self, variant: &syn::PatTupleStruct) {
        self.bind_pattern(&syn::Pat::TupleStruct(variant.clone()));
    }
    /// Records each field a struct pattern takes out of one value.
    ///
    /// The pattern writes both the type it matches and the field each
    /// name is bound to, so the answer is the field itself — the same
    /// answer a written field expression gives. What the field holds is
    /// settled wherever that type is constructed, which this tier does
    /// not read, and the answer says exactly that much.
    fn record_named_parts(&mut self, written: &syn::PatStruct) -> bool {
        let Some(owner) = written.path.segments.last() else {
            return false;
        };
        let owner = owner.ident.to_string();
        for field in &written.fields {
            let syn::Member::Named(named) = &field.member else {
                continue;
            };
            let Some(bound) = super::naming::binding_name(&field.pat) else {
                continue;
            };
            if !self.sole.contains(&bound) || self.origins.contains_key(&bound) {
                continue;
            }
            self.origins
                .insert(bound, Provenance::Field(format!("{owner}.{named}")));
        }
        true
    }

    /// Records the element a written tuple position takes out of a value.
    ///
    /// `pair.0` names position zero of `pair`, which is the selection an
    /// index at a written position already is, and the language
    /// guarantees the correspondence. A named field is not this: it is
    /// answered by the field reading, which names the type holding it.
    pub(super) fn record_indexed_part(&mut self, field: &syn::ExprField) {
        use syn::spanned::Spanned;

        let syn::Member::Unnamed(position) = &field.member else {
            return;
        };
        let operands = vec![
            self.origin_of(&field.base),
            Provenance::Written(Literal::Integer(position.index.to_string())),
        ];
        self.push_operation(
            &field.span().byte_range(),
            Operation::Selection,
            operands,
            Guarding::Unchecked
        );
    }

    /// Records the conversion a written cast performs, whose target is
    /// the type the compiler settled for the cast itself.
    ///
    /// The written target names what the source spelled; through an alias
    /// or an import that renames it, that spelling and the type reached
    /// are two different things, and only the second is the conversion.
    pub(super) fn record_cast(&mut self, cast: &syn::ExprCast) {
        use syn::spanned::Spanned;

        let operand = self.origin_of(&cast.expr);
        let target = self.settled_type_over(&cast.span().byte_range());
        let index = self.push_operation(
            &cast.span().byte_range(),
            Operation::Conversion(Conversion {
                target: target.clone()
            }),
            vec![operand],
            Guarding::Unchecked
        );
        if let Some(record) = self.facts.operations.get_mut(index) {
            record.produced = target;
        }
    }

    /// Records the operation a library method performs, where a row names
    /// one.
    pub(super) fn record_method_operation(&mut self, call: &syn::ExprMethodCall) -> bool {
        use syn::spanned::Spanned;

        let method = call.method.to_string();
        let mut operands = vec![self.origin_of(&call.receiver)];
        operands.extend(call.args.iter().map(|argument| self.origin_of(argument)));
        if let Some((operation, guarding)) = operation_method(&method) {
            self.push_operation(&call.span().byte_range(), operation, operands, guarding);
            return true;
        }
        if crate::semantic::std_knowledge::selects_an_element(&method) {
            self.push_operation(
                &call.span().byte_range(),
                Operation::Selection,
                operands,
                Guarding::Unchecked
            );
            return true;
        }
        if let Some(question) = crate::semantic::std_knowledge::presence_method(&method) {
            self.push_operation(
                &call.span().byte_range(),
                Operation::Presence(question),
                operands,
                Guarding::Unchecked
            );
            return true;
        }
        self.record_conversion(call, operands)
    }
}

/// The operation a binary operator performs, where it is one this channel
/// recognizes. An assignment writes rather than computes, and a logical
/// connective decides an order of evaluation rather than producing a
/// value from two.
const fn binary_operation(operator: syn::BinOp) -> Option<Operation> {
    Some(match operator {
        syn::BinOp::Add(_) => Operation::Arithmetic(Arithmetic::Add),
        syn::BinOp::Sub(_) => Operation::Arithmetic(Arithmetic::Subtract),
        syn::BinOp::Mul(_) => Operation::Arithmetic(Arithmetic::Multiply),
        syn::BinOp::Div(_) => Operation::Arithmetic(Arithmetic::Divide),
        syn::BinOp::Rem(_) => Operation::Arithmetic(Arithmetic::Remainder),
        syn::BinOp::Eq(_) => Operation::Comparison(Comparison::Equal),
        syn::BinOp::Ne(_) => Operation::Comparison(Comparison::NotEqual),
        syn::BinOp::Lt(_) => Operation::Comparison(Comparison::Less),
        syn::BinOp::Le(_) => Operation::Comparison(Comparison::LessOrEqual),
        syn::BinOp::Gt(_) => Operation::Comparison(Comparison::Greater),
        syn::BinOp::Ge(_) => Operation::Comparison(Comparison::GreaterOrEqual),
        syn::BinOp::And(_) => Operation::Junction(Junction::Both),
        syn::BinOp::Or(_) => Operation::Junction(Junction::Either),
        _ => return None
    })
}

/// Whether a variant pattern names the failure half of a fallible
/// answer.
///
/// The two halves are taken apart the same way and stand for different
/// values, so the position written for the failure half is one rather
/// than zero: the composition reads the half from it and never hands a
/// reader the other one.
fn names_the_failure_half(variant: &syn::PatTupleStruct) -> bool {
    variant
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Err")
}

/// The pattern under the wrappers that change how it is written but not
/// what it binds: a guard, a parenthesis, a borrow, a written type.
///
/// A guarded arm is one of these: the language writes `value if test`,
/// and the parser hands back a pattern wrapping the one that binds.
fn through_wrappers(pattern: &syn::Pat) -> &syn::Pat {
    match pattern {
        syn::Pat::Guard(guarded) => through_wrappers(&guarded.pat),
        syn::Pat::Paren(inner) => through_wrappers(&inner.pat),
        syn::Pat::Reference(borrowed) => through_wrappers(&borrowed.pat),
        syn::Pat::Type(typed) => through_wrappers(&typed.pat),
        other => other
    }
}
