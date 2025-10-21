use crate::dsp::{PolyBlepOsc, Wave as DWav, ZdfSvf, FilterMode as DMode, fast_tanh};
use crate::params::{FilterMode, WaveType};
use nih_plug::prelude::*;

pub struct Voice {
    pub active: bool,
    pub note: u8,
    pub channel: u8,
    pub note_id: Option<i32>,
    pub freq: f32,
    pub vel: f32,

    pub env: Adsr,
    pub osc1: PolyBlepOsc,
    pub osc2: PolyBlepOsc,
    pub tri_accum: f32,
    pub filt: ZdfSvf,
    pub releasing: bool,

    // Per-voice normalized offsets from PolyModulation
    pub poly_gain_norm: f32,
    pub poly_cut_norm: f32,

    // For stealing
    pub age: u64,
}

pub struct Adsr {
    sr: f32,
    a: f32, d: f32, s: f32, r: f32, // in samples
    level: f32,
    state: u8, // 0=idle 1=att 2=dec 3=sus 4=rel
}
impl Adsr {
    pub fn new(sr: f32) -> Self {
        Self { sr, a:0.0,d:0.0,s:0.7,r:0.2, level:0.0, state:0 }
    }
    pub fn set_ms(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.a = (a.max(0.0) / 1000.0) * self.sr;
        self.d = (d.max(0.0) / 1000.0) * self.sr;
        self.s = s.clamp(0.0, 1.0);
        self.r = (r.max(0.0) / 1000.0) * self.sr;
    }
    pub fn note_on(&mut self) { self.state = 1; }
    pub fn note_off(&mut self) { if self.state != 0 { self.state = 4; } }

    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.state {
            0 => { self.level = 0.0; }
            1 => {
                let inc = if self.a <= 1.0 { 1.0 } else { 1.0 / self.a };
                self.level += inc;
                if self.level >= 1.0 { self.level = 1.0; self.state = 2; }
            }
            2 => {
                let dec = if self.d <= 1.0 { 1.0 } else { 1.0 / self.d };
                self.level -= dec;
                if self.level <= self.s { self.level = self.s; self.state = 3; }
            }
            3 => {}
            4 => {
                let rel = if self.r <= 1.0 { 1.0 } else { 1.0 / self.r };
                self.level -= rel;
                if self.level <= 0.0 { self.level = 0.0; self.state = 0; }
            }
            _ => {}
        }
        self.level.clamp(0.0, 1.0)
    }
    pub fn is_idle(&self) -> bool { self.state == 0 || self.level <= 0.0 }
}

impl Voice {
    pub fn new(sr: f32) -> Self {
        Self {
            active: false,
            note: 0,
            channel: 0,
            note_id: None,
            freq: 0.0,
            vel: 0.0,
            env: Adsr::new(sr),
            osc1: PolyBlepOsc::new(sr, DWav::Saw),
            osc2: PolyBlepOsc::new(sr, DWav::Saw),
            tri_accum: 0.0,
            filt: ZdfSvf::new(sr),
            releasing: false,
            poly_gain_norm: 0.0,
            poly_cut_norm: 0.0,
            age: 0,
        }
    }

    #[inline]
    fn map_wave(w: WaveType) -> DWav {
        match w {
            WaveType::Sine => DWav::Sine,
            WaveType::Saw => DWav::Saw,
            WaveType::Square | WaveType::Triangle => DWav::Square, // tri built from band-limited square
        }
    }
    #[inline]
    fn map_mode(m: FilterMode) -> DMode {
        match m {
            FilterMode::Off => DMode::Off,
            FilterMode::LowPass => DMode::LP,
            FilterMode::BandPass => DMode::BP,
            FilterMode::HighPass => DMode::HP,
        }
    }

    pub fn start(&mut self, channel: u8, note: u8, velocity: f32, wave: WaveType, detune_cents: f32, sr: f32) {
        self.active = true;
        self.channel = channel;
        self.note = note;
        self.note_id = None;
        self.vel = velocity;
        self.freq = nih_plug::util::midi_note_to_freq(note);
        self.env.note_on();

        let w = Self::map_wave(wave);
        self.osc1.wave = w;
        self.osc2.wave = w;
        self.osc1.sr = sr; self.osc2.sr = sr;
        self.osc1.phase = 0.0; self.osc2.phase = 0.5;
        self.osc1.set_freq(self.freq);
        let dt = self.freq * (2f32.powf(detune_cents / 1200.0) - 1.0);
        self.osc2.set_freq((self.freq + dt).max(1.0));

        self.releasing = false;
        self.poly_gain_norm = 0.0;
        self.poly_cut_norm = 0.0;
        self.tri_accum = 0.0;
    }

    pub fn release(&mut self) {
        if self.active && !self.releasing {
            self.releasing = true;
            self.env.note_off();
        }
    }

    pub fn set_filter(&mut self, q: f32, mode: FilterMode, sr: f32, poly_cut_norm: f32, cutoff_param: &FloatParam) {
        // Build a per-voice cutoff using normalized offset and NIH-plug helper
        let cutoff_plain: f32 = cutoff_param.preview_modulated(poly_cut_norm);
        let cutoff_hz = cutoff_plain.clamp(20.0, sr * 0.49);
        self.filt.set(cutoff_hz, q, Self::map_mode(mode));
    }

    #[inline]
    fn triangle_from_square(&mut self, sq: f32, freq: f32, sr: f32) -> f32 {
        // Integrate BL square with a small leak to stabilize DC
        let k = (freq / sr).clamp(0.0001, 0.45);
        self.tri_accum += (2.0 * k) * sq;
        self.tri_accum *= 0.9995; // leak
        self.tri_accum.clamp(-1.2, 1.2) * 0.8
    }

    pub fn render(&mut self, wave: WaveType, osc_mix: f32, gain_plain: f32) -> f32 {
        let e = self.env.next();
        if self.env.is_idle() {
            self.active = false;
            self.releasing = false;
            return 0.0;
        }

        let s1 = match wave {
            WaveType::Sine => self.osc1.next_sine(),
            WaveType::Saw  => self.osc1.next_saw_blep(),
            _              => self.osc1.next_square_blep(),
        };
        let s2 = match wave {
            WaveType::Sine => self.osc2.next_sine(),
            WaveType::Saw  => self.osc2.next_saw_blep(),
            _              => self.osc2.next_square_blep(),
        };

        let mut osc = s1 * (1.0 - osc_mix) + s2 * osc_mix;
        if matches!(wave, WaveType::Triangle) {
            osc = self.triangle_from_square(osc, self.freq, self.osc1.sr);
        }

        // Filter -> gain -> soft clip
        let y = self.filt.process(osc * e * self.vel);
        fast_tanh(y * gain_plain)
    }
}
