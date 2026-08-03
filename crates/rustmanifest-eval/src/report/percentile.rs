// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Percentile computation over the per-file timing samples of a run.

/// Computes the p95 of `samples` in milliseconds.
///
/// The percentile uses the nearest-rank method: for `n` samples the p95 is
/// the value at rank `ceil(0.95 * n)` (1-based) of the ascending-sorted
/// vector. An empty slice yields `0.0`.
#[must_use]
pub fn p95(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = (samples.len() * 95).div_ceil(100).max(1);
    sorted.get(rank - 1).copied().unwrap_or(0.0)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code is allowed to fail loudly on broken invariants and \
              compares exact float constants"
)]
mod tests {
    use super::*;

    #[test]
    fn p95_of_known_vector_is_nearest_rank() {
        let samples: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(p95(&samples), 95.0);
    }

    #[test]
    fn p95_of_small_vectors() {
        assert_eq!(p95(&[]), 0.0);
        assert_eq!(p95(&[7.0]), 7.0);
        assert_eq!(p95(&[3.0, 1.0, 2.0]), 3.0);
        let twenty: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(p95(&twenty), 19.0);
    }
}
