//! Brown-noise generator: unit-variance white noise through a one-pole low-pass.
//!
//! This is the same filter the Electron and Python versions used
//! (`y[n] = x[n] + alpha * y[n-1]`, normalized by `1 / sqrt(1 - alpha^2)`), with
//! two changes: the white-noise source is cheaper, and `alpha` adapts to the
//! device sample rate so the output sounds identical at 44.1 kHz and 48 kHz.

/// Filter coefficient from the original implementations, which hard-coded a
/// 44.1 kHz stream. Corresponds to a corner frequency of about 35 Hz.
const REFERENCE_ALPHA: f32 = 0.995;
const REFERENCE_RATE: f32 = 44_100.0;

const SQRT_3: f32 = 1.732_050_8;

/// Rescale the reference coefficient to `sample_rate`, holding the corner
/// frequency constant.
///
/// `alpha = exp(-2*pi*fc/fs)`, so `alpha_new = alpha_ref^(fs_ref/fs_new)`.
fn alpha_for(sample_rate: f32) -> f32 {
    REFERENCE_ALPHA.powf(REFERENCE_RATE / sample_rate)
}

pub struct BrownNoise {
    alpha: f32,
    expected_std: f32,
    state: f32,
    rng: u32,
}

impl BrownNoise {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let alpha = alpha_for(sample_rate);
        Self {
            alpha,
            expected_std: (1.0 - alpha * alpha).sqrt().recip(),
            state: 0.0,
            // xorshift is degenerate at zero.
            rng: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }

    /// Uniform white noise scaled to unit variance.
    ///
    /// The originals drew Gaussian samples via Box-Muller — two `random()` calls,
    /// a `ln` and a `cos` for every sample. After the low-pass in
    /// [`Self::next_sample`] the input distribution is inaudible, so a xorshift
    /// uniform does the same job far more cheaply. The `sqrt(3)` scaling keeps
    /// variance at 1.0, which is what `expected_std` assumes, so output level
    /// matches the Electron build exactly.
    #[inline]
    fn white(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;

        const SCALE: f32 = 2.0 * SQRT_3 / 4_294_967_296.0;
        (x as f32) * SCALE - SQRT_3
    }

    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        self.state = self.white() + self.alpha * self.state;
        self.state / self.expected_std
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_matches_reference_at_reference_rate() {
        assert!((alpha_for(REFERENCE_RATE) - REFERENCE_ALPHA).abs() < 1e-6);
    }

    #[test]
    fn alpha_holds_corner_frequency_across_rates() {
        // fc = -ln(alpha) * fs / 2pi should stay constant.
        let corner = |fs: f32| -alpha_for(fs).ln() * fs / std::f32::consts::TAU;
        let reference = corner(44_100.0);
        for rate in [22_050.0, 48_000.0, 96_000.0, 192_000.0] {
            assert!(
                (corner(rate) - reference).abs() < 0.01,
                "corner frequency drifted at {rate} Hz: {} vs {reference}",
                corner(rate)
            );
        }
    }

    #[test]
    fn white_noise_has_unit_variance() {
        let mut n = BrownNoise::new(44_100.0, 1);
        let count = 200_000;
        let mut sum = 0.0f64;
        let mut sum_sq = 0.0f64;
        for _ in 0..count {
            let s = f64::from(n.white());
            sum += s;
            sum_sq += s * s;
        }
        let mean = sum / f64::from(count);
        let variance = sum_sq / f64::from(count) - mean * mean;
        assert!(mean.abs() < 0.02, "white noise is not centered: {mean}");
        assert!(
            (variance - 1.0).abs() < 0.02,
            "white noise variance is {variance}, expected 1.0"
        );
    }

    #[test]
    fn brown_output_is_normalized_and_bounded() {
        let mut n = BrownNoise::new(44_100.0, 0xDEAD_BEEF);
        // Let the filter settle before measuring.
        for _ in 0..50_000 {
            n.next_sample();
        }

        let count = 500_000;
        let mut sum_sq = 0.0f64;
        let mut peak = 0.0f32;
        for _ in 0..count {
            let s = n.next_sample();
            sum_sq += f64::from(s) * f64::from(s);
            peak = peak.max(s.abs());
        }
        let rms = (sum_sq / f64::from(count)).sqrt();

        // Normalization targets unit RMS; the filter is the only thing shaping it.
        assert!(
            (rms - 1.0).abs() < 0.15,
            "brown noise RMS is {rms}, expected roughly 1.0"
        );
        // A stable one-pole cannot run away, but assert it to catch sign errors.
        assert!(peak < 8.0, "brown noise peaked at {peak}, filter may be unstable");
    }

    #[test]
    fn independent_seeds_decorrelate_channels() {
        let mut left = BrownNoise::new(44_100.0, 0x1234_5678);
        let mut right = BrownNoise::new(44_100.0, 0x9ABC_DEF1);
        let identical = (0..10_000)
            .filter(|_| (left.next_sample() - right.next_sample()).abs() < 1e-9)
            .count();
        assert!(identical < 10, "channels are correlated ({identical} identical samples)");
    }
}
