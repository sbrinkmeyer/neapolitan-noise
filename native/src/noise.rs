//! Noise generators for the three Neapolitan flavors.
//!
//! - Vanilla: white noise, flat spectrum.
//! - Chocolate: brown noise, -6 dB/octave. The original Brownie sound.
//! - Strawberry: pink noise, -3 dB/octave.
//!
//! The brown filter is the same one the Electron and Python versions used
//! (`y[n] = x[n] + alpha * y[n-1]`, normalized by `1 / sqrt(1 - alpha^2)`), with
//! two changes: the white-noise source is cheaper, and `alpha` adapts to the
//! device sample rate so it sounds identical at 44.1 kHz and 48 kHz.

/// Brown filter coefficient from the original implementations, which hard-coded
/// a 44.1 kHz stream. Corresponds to a corner frequency of about 35 Hz.
const REFERENCE_ALPHA: f32 = 0.995;
const REFERENCE_RATE: f32 = 44_100.0;

const SQRT_3: f32 = 1.732_050_8;

/// Normalization for the pink filter below, chosen so its output lands at
/// roughly unit RMS like the other two flavors. Verified by
/// `pink_output_is_normalized`.
const PINK_NORM: f32 = 0.3275;

/// Per-flavor level trim, in linear gain.
///
/// Equal RMS does not mean equal loudness: at matched RMS, white noise is
/// perceptibly harsher and louder than brown because its energy sits where the
/// ear is most sensitive. These pull white and pink down so that rolling through
/// the flavors at a fixed volume does not jump in level.
const VANILLA_TRIM: f32 = 0.5;
const STRAWBERRY_TRIM: f32 = 0.71;
const CHOCOLATE_TRIM: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Flavor {
    Vanilla,
    Chocolate,
    Strawberry,
}

impl Flavor {
    /// Every flavor, for exhaustive checks. The UI orders them by
    /// `app::FLAVOR_STACK` instead, which is the ice-cream band order.
    #[cfg(test)]
    pub const ALL: [Self; 3] = [Self::Vanilla, Self::Chocolate, Self::Strawberry];

    pub fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Chocolate => "chocolate",
            Self::Strawberry => "strawberry",
        }
    }

    pub fn noise_name(self) -> &'static str {
        match self {
            Self::Vanilla => "white noise",
            Self::Chocolate => "brown noise",
            Self::Strawberry => "pink noise",
        }
    }

    fn trim(self) -> f32 {
        match self {
            Self::Vanilla => VANILLA_TRIM,
            Self::Chocolate => CHOCOLATE_TRIM,
            Self::Strawberry => STRAWBERRY_TRIM,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::Vanilla => 0,
            Self::Chocolate => 1,
            Self::Strawberry => 2,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Vanilla,
            2 => Self::Strawberry,
            _ => Self::Chocolate,
        }
    }
}

impl Default for Flavor {
    fn default() -> Self {
        Self::Chocolate
    }
}

/// Rescale the brown coefficient to `sample_rate`, holding the corner frequency
/// constant.
///
/// `alpha = exp(-2*pi*fc/fs)`, so `alpha_new = alpha_ref^(fs_ref/fs_new)`.
fn alpha_for(sample_rate: f32) -> f32 {
    REFERENCE_ALPHA.powf(REFERENCE_RATE / sample_rate)
}

/// One channel's worth of generator state. Holds the filter state for every
/// flavor so switching flavors mid-stream does not need to reallocate.
pub struct NoiseChannel {
    alpha: f32,
    expected_std: f32,
    brown: f32,
    /// Paul Kellett's economical pink-noise filter state.
    pink: [f32; 7],
    rng: u32,
}

impl NoiseChannel {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let alpha = alpha_for(sample_rate);
        Self {
            alpha,
            expected_std: (1.0 - alpha * alpha).sqrt().recip(),
            brown: 0.0,
            pink: [0.0; 7],
            // xorshift is degenerate at zero.
            rng: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }

    /// Uniform white noise scaled to unit variance.
    ///
    /// The originals drew Gaussian samples via Box-Muller — two `random()` calls,
    /// a `ln` and a `cos` for every sample. After the low-pass filters below the
    /// input distribution is inaudible, so a xorshift uniform does the same job
    /// far more cheaply. The `sqrt(3)` scaling keeps variance at 1.0, which is
    /// what `expected_std` assumes, so brown output level matches the Electron
    /// build exactly.
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
    fn brown(&mut self, white: f32) -> f32 {
        self.brown = white + self.alpha * self.brown;
        self.brown / self.expected_std
    }

    /// Paul Kellett's economical pink filter: a cascade of one-pole sections that
    /// approximates -3 dB/octave to within about 0.05 dB above 9 Hz.
    ///
    /// The coefficients are tuned for 44.1 kHz. At higher rates the slope shifts
    /// slightly; it stays well within "sounds like pink noise".
    #[inline]
    fn pink(&mut self, white: f32) -> f32 {
        let b = &mut self.pink;
        b[0] = 0.998_86 * b[0] + white * 0.055_517_9;
        b[1] = 0.993_32 * b[1] + white * 0.075_075_9;
        b[2] = 0.969_00 * b[2] + white * 0.153_852_0;
        b[3] = 0.866_50 * b[3] + white * 0.310_485_6;
        b[4] = 0.550_00 * b[4] + white * 0.532_952_2;
        b[5] = -0.761_6 * b[5] - white * 0.016_898_0;
        let out = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.536_2;
        b[6] = white * 0.115_926;
        out * PINK_NORM
    }

    /// Next sample for `flavor`, at unit-ish RMS before the flavor trim.
    #[inline]
    pub fn next_sample(&mut self, flavor: Flavor) -> f32 {
        let white = self.white();
        let raw = match flavor {
            Flavor::Vanilla => white,
            Flavor::Chocolate => self.brown(white),
            Flavor::Strawberry => self.pink(white),
        };
        raw * flavor.trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RMS of `count` samples of `flavor`, after letting the filters settle.
    fn rms(flavor: Flavor, count: u32, seed: u32) -> f64 {
        let mut channel = NoiseChannel::new(44_100.0, seed);
        for _ in 0..50_000 {
            channel.next_sample(flavor);
        }
        let mut sum_sq = 0.0f64;
        for _ in 0..count {
            let s = f64::from(channel.next_sample(flavor));
            sum_sq += s * s;
        }
        (sum_sq / f64::from(count)).sqrt()
    }

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
        let mut n = NoiseChannel::new(44_100.0, 1);
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
    fn brown_output_is_normalized() {
        // Untrimmed reference level: chocolate's trim is 1.0.
        let measured = rms(Flavor::Chocolate, 500_000, 0xDEAD_BEEF);
        assert!(
            (measured - 1.0).abs() < 0.15,
            "brown RMS is {measured}, expected roughly 1.0"
        );
    }

    #[test]
    fn pink_output_is_normalized() {
        // PINK_NORM exists to put pink on the same footing as the other two.
        let measured = rms(Flavor::Strawberry, 500_000, 0x1357_9BDF) / f64::from(STRAWBERRY_TRIM);
        assert!(
            (measured - 1.0).abs() < 0.15,
            "pink RMS is {measured} before trim, expected roughly 1.0 — adjust PINK_NORM"
        );
    }

    #[test]
    fn every_flavor_stays_bounded() {
        for flavor in Flavor::ALL {
            let mut channel = NoiseChannel::new(44_100.0, 0xABCD_1234);
            let mut peak = 0.0f32;
            for _ in 0..500_000 {
                peak = peak.max(channel.next_sample(flavor).abs());
            }
            assert!(
                peak < 8.0,
                "{} peaked at {peak}, filter may be unstable",
                flavor.label()
            );
        }
    }

    #[test]
    fn flavors_are_spectrally_distinct() {
        // Crude slope probe: mean absolute first difference falls as the spectrum
        // tilts toward low frequencies. white > pink > brown.
        let roughness = |flavor: Flavor| {
            let mut channel = NoiseChannel::new(44_100.0, 0x2468_ACE0);
            for _ in 0..50_000 {
                channel.next_sample(flavor);
            }
            let mut previous = channel.next_sample(flavor);
            let mut sum = 0.0f64;
            let count = 200_000;
            for _ in 0..count {
                let s = channel.next_sample(flavor);
                sum += f64::from((s - previous).abs());
                previous = s;
            }
            sum / f64::from(count)
        };

        let (white, pink, brown) = (
            roughness(Flavor::Vanilla),
            roughness(Flavor::Strawberry),
            roughness(Flavor::Chocolate),
        );
        assert!(
            white > pink && pink > brown,
            "expected white > pink > brown roughness, got {white} / {pink} / {brown}"
        );
    }

    #[test]
    fn flavor_u8_roundtrips() {
        for flavor in Flavor::ALL {
            assert_eq!(Flavor::from_u8(flavor.to_u8()), flavor);
        }
    }

    #[test]
    fn independent_seeds_decorrelate_channels() {
        let mut left = NoiseChannel::new(44_100.0, 0x1234_5678);
        let mut right = NoiseChannel::new(44_100.0, 0x9ABC_DEF1);
        let identical = (0..10_000)
            .filter(|_| {
                (left.next_sample(Flavor::Chocolate) - right.next_sample(Flavor::Chocolate)).abs()
                    < 1e-9
            })
            .count();
        assert!(
            identical < 10,
            "channels are correlated ({identical} identical samples)"
        );
    }
}
