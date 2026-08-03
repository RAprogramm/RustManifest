// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where a write sits in the text it was read from: the line it occupies
//! and the file name the diagnostic header carries.

use rustmanifest_engine::{OutputClaim, Source};

/// The line the write sits on, with the column and width of its own
/// bytes.
pub(super) fn span(
    claim: &OutputClaim,
    sources: &[Source]
) -> Option<(usize, String, usize, usize)> {
    let source = sources
        .iter()
        .find(|source| source.path().to_string_lossy() == claim.file)?;
    let text = source.text();
    let start = claim.write.site.start as usize;
    let end = (claim.write.site.end as usize).min(text.len());
    let line_start = text
        .get(..start)?
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let line_end = text
        .get(line_start..)
        .and_then(|rest| rest.find('\n'))
        .map_or(text.len(), |offset| line_start + offset);
    let line_text = text.get(line_start..line_end)?.to_owned();
    let line_number = text.get(..start)?.matches('\n').count() + 1;
    Some((
        line_number,
        line_text,
        start - line_start,
        end.saturating_sub(start)
    ))
}

/// File of a claim, shortened to its last two components so the header
/// stays readable.
pub(super) fn relative_file(claim: &OutputClaim) -> String {
    let parts: Vec<&str> = claim.file.split('/').collect();
    parts
        .len()
        .checked_sub(2)
        .map_or_else(|| claim.file.clone(), |index| parts[index..].join("/"))
}
