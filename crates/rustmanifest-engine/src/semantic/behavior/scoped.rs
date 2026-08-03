// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Proximity-scoped typing table: every observation is kept together
//! with the source file it came from, so a short key observed twice with
//! different values still resolves for a consumer close enough to one of
//! them instead of being dropped outright.

/// Typing table keeping every observation with its source file, so a
/// colliding short key resolves by the consumer's proximity — the same
/// file first, then the same crate — instead of being dropped. A
/// collision proximity cannot settle stays unresolved, never guessed.
#[derive(Debug, Clone)]
pub(super) struct Scoped<V> {
    /// Observations per short key, each with its source index.
    entries: std::collections::BTreeMap<String, Vec<(usize, V)>>
}

impl<V> Default for Scoped<V> {
    fn default() -> Self {
        Self {
            entries: std::collections::BTreeMap::new()
        }
    }
}

impl<V: Clone + PartialEq> Scoped<V> {
    /// Records one observation of `key` from `source`.
    pub(super) fn note(&mut self, key: String, source: usize, value: V) {
        let observations = self.entries.entry(key).or_default();
        if !observations
            .iter()
            .any(|(seen, known)| *seen == source && *known == value)
        {
            observations.push((source, value));
        }
    }

    /// Resolves `key` for a consumer in `source`: a unique observation
    /// wins outright; among colliding ones, a single distinct value
    /// from the consumer's file wins, then a single distinct value
    /// from the consumer's crate.
    pub(super) fn resolve(&self, key: &str, source: usize, crates: &[String]) -> Option<&V> {
        let observations = self.entries.get(key)?;
        if let [(_, only)] = observations.as_slice() {
            return Some(only);
        }
        single_value(observations.iter().filter(|(origin, _)| *origin == source)).or_else(|| {
            let own = crates.get(source)?;
            single_value(
                observations
                    .iter()
                    .filter(|(origin, _)| crates.get(*origin).is_some_and(|theirs| theirs == own))
            )
        })
    }
}

/// The single distinct value among the observations, if exactly one.
fn single_value<'observations, V: PartialEq>(
    observations: impl Iterator<Item = &'observations (usize, V)>
) -> Option<&'observations V> {
    let mut found: Option<&V> = None;
    for (_, value) in observations {
        match found {
            None => found = Some(value),
            Some(seen) if seen == value => {}
            Some(_) => return None
        }
    }
    found
}
