# Neapolitan Noise 3.0

Three flavors of noise behind a thin single-row interface: white, brown and pink.
Native, no runtime to install, one self-contained binary per platform.

Neapolitan Noise 3.0 is a Rust rewrite (`native/`) of Brownie, the Electron app
that came before it. Same brown-noise filter, plus two more scoops — and 4 MB
instead of 264 MB.

## Features

- Neapolitan flavor picker: three stacked scoops, chocolate on top
  - chocolate: brown noise, -6 dB/octave. The original Brownie sound
  - vanilla: white noise, flat spectrum
  - strawberry: pink noise, -3 dB/octave
  - Click a scoop, or focus the stack and use Up/Down. The whole palette swaps
    to match the flavor
- Stereo/Mono mode button
  - Lit: stereo (independent L/R channels)
  - Dim: mono (dual-mono output)
- Volume slider (0-99)
  - Drag the thumb, or scroll the mouse wheel over it
  - Left/Right arrows when focused
- Volume number entry
  - Click and type an exact value; Up/Down arrows when focused
- Playback button
  - Lit ON: noise playing
  - Dim OFF: stopped
- Help panel behind `?`
- Settings (volume, mono/stereo) persist between runs

Keyboard: Tab walks the controls left to right, Space or Enter flips the focused
button.

## Downloads

Grab an installer from [Releases](https://github.com/sbrinkmeyer/neapolitan-noise/releases):

| Platform | Asset |
| --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `.dmg` (universal binary) |
| Windows 10+ x64 | `.exe` |
| Linux x86_64, glibc 2.35+ | `.AppImage` |

Builds are unsigned. See [macOS Gatekeeper](#macos-gatekeeper-unsigned-build)
below.

## Build from source

Requires a [Rust](https://rustup.rs) toolchain. No other tooling on macOS or
Windows.

```bash
cargo build --release --manifest-path native/Cargo.toml
cargo test --release --manifest-path native/Cargo.toml
./native/target/release/neaponoise
```

On Linux, install the audio and windowing headers first:

```bash
sudo apt-get install -y pkg-config libasound2-dev libgl1-mesa-dev \
  libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxcursor-dev libxrandr-dev libxi-dev
```

## Build installers

Locally, for the platform you are on:

```bash
# macOS: .app bundle + DMG, named for the architectures it contains
bash packaging/macos/bundle.sh native/target/release/neaponoise 3.0.0 dist-native

# Linux: AppImage
bash packaging/linux/appimage.sh native/target/release/neaponoise 3.0.0 dist-native
```

Cross-compiling these is more trouble than it is worth; CI does it instead.

### Releases via CI

`.github/workflows/native-release.yml` builds all three platforms in parallel,
each on its own runner OS:

- macOS runner: arm64 + x64, fused into one universal binary with `lipo`, then
  bundled and ad-hoc signed
- Windows runner: single portable `.exe` with an embedded icon
- Linux runner (ubuntu-22.04, pinned to keep the glibc floor low): AppImage

Push a tag to publish:

```bash
git tag v3.0.0 && git push origin v3.0.0
```

A release is created with all artifacts attached. Opening a PR that touches
`native/` or `packaging/` runs the same three builds without publishing.

## Project structure

- `native/src/noise.rs`: white/brown/pink DSP, with tests
- `native/src/audio.rs`: cpal output stream, gain ramping
- `native/src/app.rs`: UI, flavor picker, persistence
- `native/src/palette.rs`: one color scheme per flavor, with contrast tests
- `native/src/main.rs`: window setup
- `packaging/`: per-platform bundling scripts
- `tutorial.md`: conceptual noise-generation notes

### History

Versions 1 and 2 were a Python/tkinter prototype and an Electron app. Both were
removed in 3.0 — this is now a Rust-only project. To read them:

```bash
git show v2.0.0:main.js         # Electron main process
git show v2.0.0:renderer.js     # audio + UI
git show v2.0.0:brownie.py      # the original prototype
```

Their installers are still attached to the
[v2.0.0 release](https://github.com/sbrinkmeyer/neapolitan-noise/releases/tag/v2.0.0).

## Notes on the port

Three things changed from the Electron implementation on purpose:

- **White-noise source.** The originals drew Gaussian samples via Box-Muller
  (two RNG calls, a `ln` and a `cos` per sample). After the low-pass the input
  distribution is inaudible, so a xorshift uniform scaled to unit variance does
  the same job far more cheaply. Output level is unchanged.
- **Sample-rate-independent filter.** `alpha = 0.995` was tuned for 44.1 kHz. It
  is now rescaled to the device's actual rate so the corner frequency stays at
  ~35 Hz at 48 kHz and above.
- **Gain ramping.** Volume changes glide over 25 ms instead of jumping per
  buffer, which removes the zipper noise on the slider and the click on
  start/stop.

## macOS Gatekeeper (unsigned build)

Builds are ad-hoc signed but not notarized, so macOS will say the developer
cannot be verified. Only do this for builds you trust.

### Option 1: Finder (recommended)

1. Move the app to Applications.
2. Right-click the app and choose Open.
3. Click Open again in the confirmation dialog.

After the first successful launch, macOS allows normal double-click opening.

### Option 2: Terminal (remove quarantine flag)

```bash
xattr -dr com.apple.quarantine "/Applications/Neapolitan Noise.app"
```

## Windows SmartScreen (unsigned build)

SmartScreen may show "Windows protected your PC". Choose More info, then Run
anyway.
