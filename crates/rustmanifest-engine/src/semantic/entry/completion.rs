// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the entry's own signature decides about the status the process
//! leaves behind.

/// What the entry's own signature decides about the status the process
/// leaves behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// The entry returns nothing, so reaching its end is a success and no
    /// failure inside it can leave as a status.
    Nothing,
    /// The entry returns a status value, so the status is whichever value
    /// it hands back.
    Status,
    /// The entry returns a result, so the library turns the ok value into
    /// success and the error into a failure it also reports.
    Fallible
}

/// What a written return type decides about the status the process leaves
/// behind.
///
/// The named type is read as a status only where it is the library's own
/// exit-status type. Everything else written is read as a value the
/// library turns into a status by the rule it applies to a result, which
/// is the same rule whatever the type is called.
pub(super) fn completion_of(output: &syn::ReturnType) -> Completion {
    let syn::ReturnType::Type(_, written) = output else {
        return Completion::Nothing;
    };
    let syn::Type::Path(path) = written.as_ref() else {
        return Completion::Fallible;
    };
    match path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .as_deref()
    {
        Some("ExitCode") => Completion::Status,
        None => Completion::Nothing,
        Some(_) => Completion::Fallible
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
    use super::{Completion, completion_of};

    fn read(signature: &str) -> Completion {
        let function: syn::ItemFn =
            syn::parse_str(&format!("{signature} {{ unimplemented!() }}")).expect("parses");
        completion_of(&function.sig.output)
    }

    #[test]
    fn each_written_return_names_the_rule_the_library_will_apply() {
        assert_eq!(read("fn main()"), Completion::Nothing);
        assert_eq!(read("fn main() -> ExitCode"), Completion::Status);
        assert_eq!(
            read("fn main() -> std::process::ExitCode"),
            Completion::Status
        );
        assert_eq!(read("fn main() -> Result<(), Error>"), Completion::Fallible);
    }
}
