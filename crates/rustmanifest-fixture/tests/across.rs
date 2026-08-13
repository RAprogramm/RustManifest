// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! A snippet reaching across files is one package to the compiler.

/// The type a field holds is the compiler's answer even when the type
/// stands in another file of the same fixture.
#[test]
fn a_field_typed_from_another_file_is_settled() {
    let held = rustmanifest_fixture::settled(&[
        ("src/file0.rs", "pub struct Identity { pub id: u64 }"),
        (
            "src/file1.rs",
            "pub struct User { identity: crate::file0::Identity, age: u8 }"
        )
    ]);
    assert_eq!(
        held.declared
            .iter()
            .map(|row| (
                row.named.clone(),
                row.fields
                    .iter()
                    .map(|field| (field.name.clone(), field.named.clone()))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        Vec::new()
    );
}
