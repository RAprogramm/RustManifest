// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What each written path names.
//!
//! A written path is not an identity. Two crates may both declare `Error`,
//! an import may rename one of them, and the same four letters mean a
//! different item in the next file. Keying a fact by the last segment of
//! what was typed collapses all of them into one bucket, and nothing
//! further down recovers the difference. What is recorded here is the item
//! the name reaches, spelled from its own crate root.
//!
//! Locals are deliberately not recorded. A local's identity is the binding
//! it was declared by, not a path, and a name would read as an identity
//! while telling two shadowed bindings apart no better than the text does.
//! Absence is what this shape means by "not answered"; a row that lies is
//! worse than no row.

use ra_ap_hir::{Adt, ModuleDef, PathResolution};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::{AstNode, Edition, ast};
use rustmanifest_schema::ResolvedPath;

use super::reading::Reading;

/// Records what every path written in `parsed` names, where the resolver
/// settles it to an item.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedPath>
) {
    for path in parsed.syntax().descendants().filter_map(ast::Path::cast) {
        if let Some(settled) = settled_path(reading, &path) {
            into.push(settled);
        }
    }
}

/// Reads one written path, answering `None` where it reaches no item.
fn settled_path(reading: Reading<'_, '_>, path: &ast::Path) -> Option<ResolvedPath> {
    let (definition, kind) = match reading.semantics.resolve_path(path)? {
        PathResolution::Def(definition) => (definition, kind_of(definition)),
        PathResolution::SelfType(implementation) => (
            ModuleDef::Adt(implementation.self_ty(reading.database).as_adt()?),
            "self-type"
        ),
        _ => return None
    };
    let krate = definition.module(reading.database)?.krate(reading.database);
    Some(ResolvedPath {
        at:        reading.located(path.syntax().text_range()),
        names:     canonical(
            reading.database,
            definition,
            krate.edition(reading.database)
        )?,
        kind:      kind.to_owned(),
        declaring: named(reading.database, krate)?
    })
}

/// Spells one declared type from its crate root, crate name first.
pub(super) fn canonical_type(database: &RootDatabase, declared: Adt) -> Option<String> {
    let definition = ModuleDef::Adt(declared);
    let krate = definition.module(database)?.krate(database);
    canonical(database, definition, krate.edition(database))
}

/// Spells one function from its crate root, crate name first, with the
/// type or trait it is written under kept in the path.
///
/// The module alone does not identify a method: two types declared beside
/// each other each have a `new`, and both live in that module. The
/// container is what tells them apart.
pub(super) fn canonical_function(
    database: &RootDatabase,
    function: ra_ap_hir::Function
) -> Option<String> {
    let definition = ModuleDef::Function(function);
    let krate = definition.module(database)?.krate(database);
    let path = canonical(database, definition, krate.edition(database))?;
    let Some(under) = written_under(database, function) else {
        return Some(path);
    };
    let (above, named) = path.rsplit_once("::")?;
    Some(format!("{above}::{under}::{named}"))
}

/// The type or trait a function is written under, where it is written
/// under one.
fn written_under(database: &RootDatabase, function: ra_ap_hir::Function) -> Option<String> {
    use ra_ap_hir::{AsAssocItem as _, AssocItemContainer};

    match function.as_assoc_item(database)?.container(database) {
        AssocItemContainer::Trait(held) => Some(held.name(database).as_str().to_owned()),
        AssocItemContainer::Impl(held) => Some(
            held.self_ty(database)
                .as_adt()?
                .name(database)
                .as_str()
                .to_owned()
        )
    }
}

/// Spells `definition` from its crate root, crate name first.
fn canonical(database: &RootDatabase, definition: ModuleDef, edition: Edition) -> Option<String> {
    let inside = definition.canonical_path(database, edition)?;
    let krate = named(database, definition.module(database)?.krate(database))?;
    Some(format!("{krate}::{inside}"))
}

/// Names the crate `krate`, or `None` for one with no declared name.
fn named(database: &RootDatabase, krate: ra_ap_hir::Crate) -> Option<String> {
    Some(krate.display_name(database)?.to_string())
}

/// Names the kind of item a definition is, in the vocabulary the shape
/// documents.
const fn kind_of(definition: ModuleDef) -> &'static str {
    match definition {
        ModuleDef::Module(_) => "module",
        ModuleDef::Function(_) => "function",
        ModuleDef::Adt(Adt::Struct(_)) => "struct",
        ModuleDef::Adt(Adt::Enum(_)) => "enum",
        ModuleDef::Adt(Adt::Union(_)) => "union",
        ModuleDef::EnumVariant(_) => "variant",
        ModuleDef::Const(_) => "const",
        ModuleDef::Static(_) => "static",
        ModuleDef::Trait(_) => "trait",
        ModuleDef::TypeAlias(_) => "type-alias",
        ModuleDef::BuiltinType(_) => "builtin-type",
        ModuleDef::Macro(_) => "macro"
    }
}
