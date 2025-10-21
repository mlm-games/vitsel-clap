mod dsp;
mod params;
mod presets;
mod voice;

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

use params::*;
use voice::Voice;

pub struct Vitsel {
    params: Arc<VitsParams>,
    sample_rate: f32,
    voices: Vec<Voice>,
    frame_counter: u64,
}

impl Default for Vitsel {
    fn default() -> Self {
        let params = Arc::new(VitsParams::default());
        let sr = 44100.0;
        let maxv = params.max_voices.value() as usize;
        Self {
            params,
            sample_rate: sr,
            voices: (0..maxv).map(|_| Voice::new(sr)).collect(),
            frame_counter: 0,
        }
    }
}

impl Plugin for Vitsel {
    const NAME: &'static str = "Vitsel";
    const VENDOR: &'static str = "me";
    const URL: &'static str = "https://website.com";
    const EMAIL: &'static str = "me@website.com";
    const VERSION: &'static str = "1.0.2";

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic; // to send VoiceTerminated
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _io: &AudioIOLayout,
        buffer_config: &BufferConfig,
        ctx: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        // Pre-size voices
        self.resize_voice_pool();

        // Tell host the initial capacity for CLAP poly-mod
        if <Self as ClapPlugin>::CLAP_POLY_MODULATION_CONFIG.is_some() {
            ctx.set_current_voice_capacity(self.voices.len() as u32);
        }
        true
    }

    fn reset(&mut self) {
        self.frame_counter = 0;
        for v in &mut self.voices {
            *v = Voice::new(self.sample_rate);
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer<'_>,
        _aux: &mut AuxiliaryBuffers<'_>,
        ctx: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.frame_counter = self.frame_counter.wrapping_add(1);

        let params = self.params.clone();
        let p_gain = &params.gain;
        let p_cutoff = &params.cutoff_hz;

        // Dynamically respond to voice count changes
        let desired = params.max_voices.value() as usize;
        if desired != self.voices.len() {
            self.resize_voice_pool();
            if <Self as ClapPlugin>::CLAP_POLY_MODULATION_CONFIG.is_some() {
                ctx.set_current_voice_capacity(self.voices.len() as u32);
            }
        }

        let wave = params.wave.value();
        let detune_cents = params.detune.value();
        let osc_mix = params.osc_mix.value();
        let q = 1.0f32 + (params.resonance.value() * 7.0);
        let fmode = params.filter_mode.value();

        let mut next_event = ctx.next_event();

        for (sample_idx, mut frame) in buffer.iter_samples().enumerate() {
            // Handle sample-accurate events (may call self.alloc_voice() = &mut self)
            while let Some(ev) = next_event {
                if ev.timing() != sample_idx as u32 {
                    break;
                }
                match ev {
                    NoteEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        voice_id,
                        ..
                    } => {
                        let slot = self.alloc_voice(); // OK now: no outstanding &self borrows
                        let v = &mut self.voices[slot];
                        v.start(
                            channel,
                            note,
                            velocity,
                            wave,
                            detune_cents,
                            self.sample_rate,
                        );
                        v.note_id = voice_id.map(|x| x as i32);
                        v.age = self.frame_counter;
                    }
                    NoteEvent::NoteOff {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        for v in &mut self.voices {
                            if v.active
                                && v.channel == channel
                                && v.note == note
                                && (voice_id
                                    .map(|id| v.note_id == Some(id as i32))
                                    .unwrap_or(true))
                            {
                                v.release();
                            }
                        }
                    }
                    NoteEvent::PolyModulation {
                        voice_id,
                        poly_modulation_id,
                        normalized_offset,
                        ..
                    } => {
                        for v in &mut self.voices {
                            if v.active && v.note_id == Some(voice_id as i32) {
                                match poly_modulation_id {
                                    1 => v.poly_gain_norm = normalized_offset,
                                    2 => v.poly_cut_norm = normalized_offset,
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
                next_event = ctx.next_event();
            }

            // Render
            let mut l = 0.0f32;
            let mut r = 0.0f32;

            for v in &mut self.voices {
                if !v.active {
                    continue;
                }

                v.set_filter(
                    q,
                    fmode,
                    self.sample_rate,
                    v.poly_cut_norm * params.mod_cutoff.value(),
                    p_cutoff,
                );
                let gain_plain: f32 =
                    p_gain.preview_modulated(v.poly_gain_norm * params.mod_gain.value());

                let y = v.render(wave, osc_mix, gain_plain);
                l += y;
                r += y;

                if !v.active {
                    ctx.send_event(NoteEvent::VoiceTerminated {
                        timing: sample_idx as u32,
                        voice_id: v.note_id,
                        channel: v.channel,
                        note: v.note,
                    });
                }
            }

            let scale = (1.0 / (self.voices.len().max(1) as f32).sqrt()).min(1.0);
            let out_l = l * scale;
            let out_r = r * scale;
            let mut ch = frame.iter_mut();
            if let Some(s) = ch.next() {
                *s = out_l;
            }
            if let Some(s) = ch.next() {
                *s = out_r;
            }
        }

        ProcessStatus::Normal
    }
}

impl Vitsel {
    fn resize_voice_pool(&mut self) {
        let n = self.params.max_voices.value() as usize;
        if n > self.voices.len() {
            self.voices
                .extend((self.voices.len()..n).map(|_| Voice::new(self.sample_rate)));
        } else {
            self.voices.truncate(n);
        }
    }
    fn alloc_voice(&mut self) -> usize {
        if let Some(i) = self.voices.iter().position(|v| !v.active) {
            return i;
        }
        let (mut best_i, mut best_age) = (0usize, u64::MAX);
        for (i, v) in self.voices.iter().enumerate() {
            if v.age < best_age {
                best_age = v.age;
                best_i = i;
            }
        }
        best_i
    }
}

// CLAP metadata
impl ClapPlugin for Vitsel {
    const CLAP_ID: &'static str = "dev.example.vitsel";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Minimal production-ready CLAP synth for Android/desktop.");
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
    const CLAP_POLY_MODULATION_CONFIG: Option<PolyModulationConfig> = Some(PolyModulationConfig {
        max_voice_capacity: 64,
        supports_overlapping_voices: true,
    });

    fn remote_controls(&self, context: &mut impl RemoteControlsContext) {}

    const CLAP_MANUAL_URL: Option<&'static str> = { Some("Not yet") };

    const CLAP_SUPPORT_URL: Option<&'static str> = { Some("Not yet") };
}

nih_export_clap!(Vitsel);
