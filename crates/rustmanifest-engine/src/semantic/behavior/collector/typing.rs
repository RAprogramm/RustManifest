// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type name an expression settles to, for every expression the
//! recorded tables can answer for, and the type a `let` initializer
//! written as a literal or a constructor call carries on its face.

use super::FactCollector;

impl FactCollector {
    /// Types any expression the tables can settle: bindings and fields
    /// by their recorded types, indexed collections by element, calls
    /// through [`Self::called_return`].
    ///
    /// A borrow, a parenthesis, a group and a dereference are seen
    /// through: they change how a value is held, never which value it is,
    /// and a method written on a borrow is the method of the type behind
    /// it. Reading only the bare form left every borrowed receiver
    /// untyped, which is most of them.
    pub(super) fn expr_type(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Reference(borrowed) => self.expr_type(&borrowed.expr),
            syn::Expr::Paren(inner) => self.expr_type(&inner.expr),
            syn::Expr::Group(inner) => self.expr_type(&inner.expr),
            syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
                self.expr_type(&unary.expr)
            }
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map(|name| self.bounds.get(name).unwrap_or(name).clone()),
            syn::Expr::Field(field) => {
                if let Some(settled) = self.settled_field_type(field) {
                    return Some(settled);
                }
                let owner = self.expr_type(&field.base)?;
                let member = super::naming::member_name(&field.member);
                self.field(&format!("{owner}.{member}"))
                    .map(|inner| self.bounds.get(inner).unwrap_or(inner).clone())
                    .or_else(|| {
                        crate::semantic::knowledge::field_type(&owner, &member)
                            .map(ToOwned::to_owned)
                    })
            }
            syn::Expr::Index(indexed) => self.element_of(&indexed.expr),
            syn::Expr::Match(matched) => self.arms_agree(matched),
            other => truth_of(other).or_else(|| self.called_return(other))
        }
    }

    /// The type a `match` hands back: the language requires every arm
    /// that hands a value back to hand back the same type, so one
    /// readable arm answers for the whole — and an arm that leaves the
    /// path instead of handing a value back, by returning, breaking,
    /// continuing or panicking, has no type to disagree with.
    ///
    /// The arms are read after the patterns have bound what they carry,
    /// which is what makes the common shape — a fallible call matched
    /// with its success bound and its failure returned — readable at
    /// all.
    fn arms_agree(&self, matched: &syn::ExprMatch) -> Option<String> {
        let mut agreed: Option<String> = None;
        for arm in &matched.arms {
            if leaves_the_path(&arm.body) {
                continue;
            }
            let held = self.expr_type(&arm.body)?;
            match &agreed {
                Some(first) if *first != held => return None,
                Some(_) => {}
                None => agreed = Some(held)
            }
        }
        agreed
    }
}

/// Whether an arm leaves the path instead of handing a value back.
fn leaves_the_path(expression: &syn::Expr) -> bool {
    const LEAVING_MACROS: [&str; 5] = [
        "panic",
        "todo",
        "unimplemented",
        "unreachable",
        "process_exit"
    ];
    match expression {
        syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_) => true,
        syn::Expr::Macro(written) => {
            written.mac.path.segments.last().is_some_and(|segment| {
                LEAVING_MACROS.contains(&segment.ident.to_string().as_str())
            })
        }
        syn::Expr::Block(block) => {
            block
                .block
                .stmts
                .last()
                .is_some_and(|statement| match statement {
                    syn::Stmt::Expr(inner, _) => leaves_the_path(inner),
                    _ => false
                })
        }
        _ => false
    }
}

/// The type an expression carries by its own written shape: a
/// comparison, a logical connective, or a negation yields `bool`
/// whatever its operands are, and a boolean literal says so outright.
///
/// The written form is the whole proof here — no table is consulted,
/// because the language fixes the answer. It matters for the calls
/// written on such an expression: `.then`, `.then_some`, `.not` read as
/// an untyped receiver otherwise, and a workspace method of the same
/// name would stand accused of being what the call meant.
fn truth_of(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Binary(binary) => matches!(
            binary.op,
            syn::BinOp::Eq(_)
                | syn::BinOp::Ne(_)
                | syn::BinOp::Lt(_)
                | syn::BinOp::Le(_)
                | syn::BinOp::Gt(_)
                | syn::BinOp::Ge(_)
                | syn::BinOp::And(_)
                | syn::BinOp::Or(_)
        )
        .then(|| "bool".to_owned()),
        syn::Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Not(_)).then(|| "bool".to_owned())
        }
        syn::Expr::Lit(written) => {
            matches!(written.lit, syn::Lit::Bool(_)).then(|| "bool".to_owned())
        }
        _ => None
    }
}

/// Infers the type name of a `let` initializer: a struct literal or a
/// `Type::new(..)` / `Type::default()` constructor call.
pub(super) fn initializer_type(expression: &syn::Expr) -> Option<String> {
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
