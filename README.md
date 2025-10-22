# Vitsel

A small Rust CLAP synth that runs headless on Android (for yadaw) and on desktop CLAP hosts.

Highlights
- PolyBLEP oscillators: sine, saw, square; triangle via integrated BL square
- ADSR, zero-delay TPT SVF (LP/BP/HP)
- Polyphonic modulation for Gain and Cutoff (CLAP), sample-accurate events, VoiceTerminated
- No allocations or locks in the audio thread, denormal guards, soft-clipper safety
- 32–64 voices with simple oldest-voice stealing
- Headless: hosts build a parameter UI (Android-friendly)

Build (Android on-device via Termux)
```sh
pkg update && pkg install -y rust clang cmake ninja pkg-config git
cargo install cargo-ndk
rustup target add aarch64-linux-android
export CARGO_NDK_ON_ANDROID=1
cargo ndk -t arm64-v8a --platform 26 build --release
cp target/aarch64-linux-android/release/libvitsel.so Vitsel.clap
```

Use with yadaw (Android)
- Put Vitsel.clap in a directory yadaw scans, e.g.:
  - /storage/emulated/0/Android/data/<your.yadaw.package>/files/plugins/clap/ (need adb or shizuku access, yadaw supports it by copying it from external to internal, since newer android devices open everything under storage/emulated/0 in noexec mode)
  - Create a folder that ends with .clap and put the .so file in the folder (compiled file from the binary, rename the .clap file to .so if you ran the cp step given above)
  - or your app-internal: /data/data/<your.yadaw.package>/files/plugins/clap/ (need root perms, similar naming scheme as above)
<!-- - In yadaw, set additional plugin search paths if you added UI to configure them. -->

Build (desktop quick)
- Linux: `cargo build --release && cp target/release/libvitsel.so Vitsel.clap`
- Windows (MSVC): `cargo build --release && copy target\release\vitsel.dll Vitsel.clap`
- macOS: `cargo build --release` then bundle as a .clap, or use NIH‑plug’s bundler (`cargo xtask bundle` if you set it up)

Notes on CLAP poly‑mod
- using normalized_offset and Param::preview_modulated() for per‑voice values and emits NoteEvent::VoiceTerminated when voices end; his plugin also sets capacity on init/resize. For more info, see NoteEvent::PolyModulation, Param, and ClapPlugin::PolyModulationConfig in NIH‑plug docs.

[License](LICENSE)
