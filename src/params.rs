use nih_plug::prelude::*;

#[derive(Params)]
pub struct VitsParams {
    // Master / voices
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "voices"]
    pub max_voices: IntParam,

    // Oscillators
    #[id = "wave"]
    pub wave: EnumParam<WaveType>,
    #[id = "osc_mix"]
    pub osc_mix: FloatParam, // 0..1 mix between osc1 and osc2
    #[id = "detune"]
    pub detune: FloatParam, // cents

    // Envelope
    #[id = "att"]
    pub attack_ms: FloatParam,
    #[id = "dec"]
    pub decay_ms: FloatParam,
    #[id = "sus"]
    pub sustain: FloatParam,
    #[id = "rel"]
    pub release_ms: FloatParam,

    // Filter
    #[id = "f_mode"]
    pub filter_mode: EnumParam<FilterMode>,
    #[id = "f_cut"]
    pub cutoff_hz: FloatParam,
    #[id = "f_res"]
    pub resonance: FloatParam,

    // Mod routing depths (monophonic scaling of poly offsets)
    #[id = "mod_cut"]
    pub mod_cutoff: FloatParam,
    #[id = "mod_gain"]
    pub mod_gain: FloatParam,
}

#[derive(PartialEq, Eq, Clone, Copy, Enum)]
pub enum WaveType {
    Sine,
    Saw,
    Square,
    Triangle,
}

#[derive(PartialEq, Eq, Clone, Copy, Enum)]
pub enum FilterMode {
    Off,
    LowPass,
    BandPass,
    HighPass,
}

impl Default for VitsParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new("Gain", 0.8, FloatRange::Linear { min: 0.0, max: 2.0 })
                .with_unit("×")
                .with_poly_modulation_id(1), // per-voice modulation id
            max_voices: IntParam::new("Voices", 32, IntRange::Linear { min: 1, max: 64 }),

            wave: EnumParam::new("Wave", WaveType::Saw),
            osc_mix: FloatParam::new("Osc Mix", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            detune: FloatParam::new(
                "Detune",
                6.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 50.0,
                },
            )
            .with_unit("cents"),

            attack_ms: FloatParam::new(
                "Attack",
                5.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 2000.0,
                    factor: 2.5,
                },
            )
            .with_unit("ms"),
            decay_ms: FloatParam::new(
                "Decay",
                80.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 4000.0,
                    factor: 2.5,
                },
            )
            .with_unit("ms"),
            sustain: FloatParam::new("Sustain", 0.7, FloatRange::Linear { min: 0.0, max: 1.0 }),
            release_ms: FloatParam::new(
                "Release",
                150.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 8000.0,
                    factor: 2.5,
                },
            )
            .with_unit("ms"),

            filter_mode: EnumParam::new("Filter", FilterMode::LowPass),
            cutoff_hz: FloatParam::new(
                "Cutoff",
                1600.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20000.0,
                    factor: 0.2,
                },
            )
            .with_unit("Hz")
            .with_poly_modulation_id(2),
            resonance: FloatParam::new("Resonance", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 }),

            mod_cutoff: FloatParam::new(
                "Mod->Cutoff",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            ),
            mod_gain: FloatParam::new(
                "Mod->Gain",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            ),
        }
    }
}

impl VitsParams {
    #[allow(clippy::too_many_arguments)]
    pub fn with_values(
        gain: f32,
        max_voices: i32,
        wave: WaveType,
        osc_mix: f32,
        detune: f32,
        attack_ms: f32,
        decay_ms: f32,
        sustain: f32,
        release_ms: f32,
        filter_mode: FilterMode,
        cutoff_hz: f32,
        resonance: f32,
        mod_cutoff: f32,
        mod_gain: f32,
    ) -> Self {
        Self {
            // Keep ranges/units and poly-mod IDs identical to Default
            gain: FloatParam::new("Gain", gain, FloatRange::Linear { min: 0.0, max: 2.0 })
                .with_unit("×")
                .with_poly_modulation_id(1),
            max_voices: IntParam::new("Voices", max_voices, IntRange::Linear { min: 1, max: 64 }),

            wave: EnumParam::new("Wave", wave),
            osc_mix: FloatParam::new(
                "Osc Mix",
                osc_mix,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            detune: FloatParam::new(
                "Detune",
                detune,
                FloatRange::Linear {
                    min: 0.0,
                    max: 50.0,
                },
            )
            .with_unit("cents"),

            attack_ms: FloatParam::new(
                "Attack",
                attack_ms,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 2000.0,
                    factor: 2.5,
                },
            )
            .with_unit("ms"),
            decay_ms: FloatParam::new(
                "Decay",
                decay_ms,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 4000.0,
                    factor: 2.5,
                },
            )
            .with_unit("ms"),
            sustain: FloatParam::new(
                "Sustain",
                sustain,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            release_ms: FloatParam::new(
                "Release",
                release_ms,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 8000.0,
                    factor: 2.5,
                },
            )
            .with_unit("ms"),

            filter_mode: EnumParam::new("Filter", filter_mode),
            cutoff_hz: FloatParam::new(
                "Cutoff",
                cutoff_hz,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20000.0,
                    factor: 0.2,
                },
            )
            .with_unit("Hz")
            .with_poly_modulation_id(2),
            resonance: FloatParam::new(
                "Resonance",
                resonance,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),

            mod_cutoff: FloatParam::new(
                "Mod->Cutoff",
                mod_cutoff,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            ),
            mod_gain: FloatParam::new(
                "Mod->Gain",
                mod_gain,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            ),
        }
    }
}
