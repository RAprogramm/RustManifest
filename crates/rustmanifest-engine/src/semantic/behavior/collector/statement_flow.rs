// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a statement proves about the statement after it, and the left
//! fold that carries those answers through a block.

use super::{
    catalog::{DIVERGING_MACROS, SETTLED_MACROS},
    completion::Completion,
    flow::Flow,
    gate::gated,
    scope::FlowScope
};

impl FlowScope<'_> {
    /// What a statement proves about the statement after it. A
    /// configuration gate weakens the answer and never decides it: the
    /// gated statement exists in some builds and not in others, so a
    /// divergence under a gate proves nothing about a build where the
    /// gate is off.
    pub(super) fn statement_flow(&self, statement: &syn::Stmt) -> Flow {
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
    pub(super) fn block_flow(&self, block: &syn::Block) -> Flow {
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
    pub(super) fn macro_flow(&self, invocation: &syn::Macro) -> Flow {
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
}
