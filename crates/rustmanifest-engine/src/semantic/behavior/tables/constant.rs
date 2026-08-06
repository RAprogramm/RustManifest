// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Named string constants of the workspace: what a constant's name stands
//! for, where its value is a string literal written in full.
//!
//! Only a literal initializer is read. A constant computed from another
//! constant, from a length, from a concatenating macro or from anything
//! else is left out — folding it would require evaluating an expression,
//! and this tier proves rather than evaluates. Absence from this table is
//! therefore "not proven a literal", never "not a constant".

use syn::visit::Visit;

use super::super::scoped::Scoped;
use crate::source::Source;

/// Reads the name of every constant, static and payload-free variant the
/// workspace declares, whatever its value is written as.
///
/// A variant carrying nothing is a named value exactly as a constant is:
/// writing it names one value, and which one is settled by the
/// declaration. It is noted under its own name and under
/// `Enum.Variant`, because either spelling may be written.
///
/// A name, not a value: that a path names a constant is what the
/// declaration proves, and what the constant holds is settled where it is
/// declared. Reading the value would mean evaluating the initializer,
/// which this tier does not do.
pub(super) fn constant_names(sources: &[Source]) -> Scoped<()> {
    let mut table = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut harvest = ConstantNames {
            source: index,
            table:  &mut table
        };
        harvest.visit_file(file);
    }
    table
}

/// Walk collecting the declared names of one source.
struct ConstantNames<'table> {
    /// Source the declarations being read belong to.
    source: usize,
    /// Table under construction.
    table:  &'table mut Scoped<()>
}

impl<'ast> Visit<'ast> for ConstantNames<'_> {
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.table.note(item.ident.to_string(), self.source, ());
        syn::visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.table.note(item.ident.to_string(), self.source, ());
        syn::visit::visit_item_static(self, item);
    }

    fn visit_impl_item_const(&mut self, item: &'ast syn::ImplItemConst) {
        self.table.note(item.ident.to_string(), self.source, ());
        syn::visit::visit_impl_item_const(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        let owner = item.ident.to_string();
        for variant in &item.variants {
            if !matches!(variant.fields, syn::Fields::Unit) {
                continue;
            }
            let named = variant.ident.to_string();
            self.table.note(format!("{owner}.{named}"), self.source, ());
            self.table.note(named, self.source, ());
        }
        syn::visit::visit_item_enum(self, item);
    }
}

/// Reads every string constant whose value is written as a literal.
pub(super) fn constant_texts(sources: &[Source]) -> Scoped<String> {
    let mut table = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut harvest = TextConstants {
            source: index,
            table:  &mut table
        };
        harvest.visit_file(file);
    }
    table
}

/// Walk collecting the string constants of one source, at any depth: a
/// constant nested in a module or in a function body names the same value
/// wherever it is read from.
struct TextConstants<'table> {
    /// Source the constants being read belong to.
    source: usize,
    /// Table under construction.
    table:  &'table mut Scoped<String>
}

impl TextConstants<'_> {
    /// Records `name` as the literal `value` stands for.
    fn note(&mut self, name: &syn::Ident, value: &syn::Expr) {
        if let Some(text) = literal_text(value) {
            self.table.note(name.to_string(), self.source, text);
        }
    }
}

impl<'ast> Visit<'ast> for TextConstants<'_> {
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.note(&item.ident, &item.expr);
        syn::visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.note(&item.ident, &item.expr);
        syn::visit::visit_item_static(self, item);
    }
}

/// The text a literal expression is written as, where it is a plain
/// string literal.
fn literal_text(expression: &syn::Expr) -> Option<String> {
    let syn::Expr::Lit(literal) = expression else {
        return None;
    };
    match &literal.lit {
        syn::Lit::Str(text) => Some(text.value()),
        _ => None
    }
}
