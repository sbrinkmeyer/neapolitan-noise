//! cpal output stream. One callback, one backend per platform: CoreAudio on
//! macOS, WASAPI on Windows, ALSA on Linux.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

use crate::noise::{Flavor, NoiseChannel};

/// Time to glide between gain targets. Removes the zipper noise the WebAudio
/// version had when dragging the slider, and the click on start/stop.
const RAMP_MS: f32 = 25.0;

/// Shared between the UI thread (writer) and the audio callback (reader).
pub struct Params {
    /// `f32` bits — atomics have no float variant.
    target_gain: AtomicU32,
    stereo: AtomicBool,
    /// [`Flavor`] as `u8`; see `Flavor::to_u8`.
    flavor: AtomicU8,
    /// Set by the callback once the ramp has actually reached zero, so the UI
    /// knows it is safe to drop the stream without a click.
    silent: AtomicBool,
}

impl Params {
    pub fn new(gain: f32, stereo: bool, flavor: Flavor) -> Self {
        Self {
            target_gain: AtomicU32::new(gain.to_bits()),
            stereo: AtomicBool::new(stereo),
            flavor: AtomicU8::new(flavor.to_u8()),
            silent: AtomicBool::new(true),
        }
    }

    pub fn set_flavor(&self, flavor: Flavor) {
        self.flavor.store(flavor.to_u8(), Ordering::Relaxed);
    }

    pub fn flavor(&self) -> Flavor {
        Flavor::from_u8(self.flavor.load(Ordering::Relaxed))
    }

    pub fn set_gain(&self, gain: f32) {
        self.target_gain
            .store(gain.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.target_gain.load(Ordering::Relaxed))
    }

    pub fn set_stereo(&self, stereo: bool) {
        self.stereo.store(stereo, Ordering::Relaxed);
    }

    pub fn stereo(&self) -> bool {
        self.stereo.load(Ordering::Relaxed)
    }

    pub fn is_silent(&self) -> bool {
        self.silent.load(Ordering::Acquire)
    }
}

/// Open the default output device and start streaming. The returned stream must
/// be kept alive; dropping it stops playback.
pub fn start(params: Arc<Params>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no audio output device found".to_owned())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("could not read output config: {e}"))?;

    let format = supported.sample_format();
    let config: StreamConfig = supported.into();

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, params),
        SampleFormat::F64 => build::<f64>(&device, config, params),
        SampleFormat::I16 => build::<i16>(&device, config, params),
        SampleFormat::I32 => build::<i32>(&device, config, params),
        SampleFormat::U16 => build::<u16>(&device, config, params),
        other => Err(format!("unsupported sample format: {other}")),
    }?;

    stream
        .play()
        .map_err(|e| format!("could not start playback: {e}"))?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    params: Arc<Params>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32> + 'static,
{
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate as f32;

    let mut left = NoiseChannel::new(sample_rate, 0x1234_5678);
    let mut right = NoiseChannel::new(sample_rate, 0x9ABC_DEF1);

    // Always fade up from silence so opening the stream cannot click.
    let mut gain = 0.0f32;
    let step = 1.0 / (RAMP_MS * 0.001 * sample_rate);

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let target = params.gain();
                let stereo = params.stereo();
                // Read once per buffer, not per sample: a flavor change lands on a
                // buffer boundary, which for a noise signal is inaudible.
                let flavor = params.flavor();

                for frame in data.chunks_mut(channels) {
                    if gain < target {
                        gain = (gain + step).min(target);
                    } else if gain > target {
                        gain = (gain - step).max(target);
                    }

                    let l = left.next_sample(flavor) * gain;
                    // Mono is dual-mono, matching the original's `right.set(left)`.
                    let r = if stereo {
                        right.next_sample(flavor) * gain
                    } else {
                        l
                    };

                    for (i, sample) in frame.iter_mut().enumerate() {
                        *sample = T::from_sample(if i % 2 == 1 { r } else { l });
                    }
                }

                params
                    .silent
                    .store(gain <= 0.0 && target <= 0.0, Ordering::Release);
            },
            |err| eprintln!("brownie: audio stream error: {err}"),
            None,
        )
        .map_err(|e| format!("could not open output stream: {e}"))
}
