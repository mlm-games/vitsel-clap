use core::f32::consts::PI;

#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    // Lightweight soft clip to prevent nasty overs
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[inline]
pub fn flush_denormals(x: f32) -> f32 {
    if x.abs() < 1e-24 { 0.0 } else { x }
}

#[derive(Clone, Copy)]
pub enum Wave { Sine, Saw, Square }

pub struct PolyBlepOsc {
    pub sr: f32,
    pub phase: f32,
    pub incr: f32,
    pub wave: Wave,
}

impl PolyBlepOsc {
    pub fn new(sr: f32, wave: Wave) -> Self {
        Self { sr, phase: 0.0, incr: 0.0, wave }
    }
    #[inline]
    pub fn set_freq(&mut self, f: f32) {
        self.incr = (f / self.sr) * 2.0 * PI;
    }

    #[inline]
    fn poly_blep(t: f32, dt: f32) -> f32 {
        if t < dt {
            let t = t / dt;
            t + t - t * t - 1.0
        } else if t > 1.0 - dt {
            let t = (t - 1.0) / dt;
            t * t + t + t + 1.0
        } else {
            0.0
        }
    }

    #[inline]
    fn t_dt(&self) -> (f32, f32) {
        let t = self.phase / (2.0 * PI);
        let dt = self.incr / (2.0 * PI);
        (t, dt)
    }

    #[inline]
    pub fn next_square_blep(&mut self) -> f32 {
        let (t, dt) = self.t_dt();
        let mut y = if t < 0.5 { 1.0 } else { -1.0 };
        y += Self::poly_blep(t, dt);
        y -= Self::poly_blep((t + 0.5) % 1.0, dt);
        self.advance();
        flush_denormals(y)
    }

    #[inline]
    pub fn next_saw_blep(&mut self) -> f32 {
        let (t, dt) = self.t_dt();
        let mut y = 2.0 * t - 1.0;
        y -= Self::poly_blep(t, dt);
        self.advance();
        flush_denormals(y)
    }

    #[inline]
    pub fn next_sine(&mut self) -> f32 {
        let y = self.phase.sin();
        self.advance();
        flush_denormals(y)
    }

    #[inline]
    fn advance(&mut self) {
        self.phase += self.incr;
        while self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
    }
}

// Zero-delay TPT state variable filter
pub struct ZdfSvf {
    sr: f32,
    ic1eq: f32,
    ic2eq: f32,
    g: f32,
    r: f32, // r = 1/Q
    mode: FilterMode,
}

#[derive(Clone, Copy)]
pub enum FilterMode { Off, LP, BP, HP }

impl ZdfSvf {
    pub fn new(sr: f32) -> Self {
        Self { sr, ic1eq: 0.0, ic2eq: 0.0, g: 0.0, r: 1.0, mode: FilterMode::LP }
    }

    #[inline]
    pub fn set(&mut self, cutoff_hz: f32, q: f32, mode: FilterMode) {
        let f = (cutoff_hz / self.sr).clamp(1e-5, 0.49);
        self.g = (PI * f).tan();
        self.r = (1.0 / q.max(0.05)).clamp(0.02, 10.0);
        self.mode = mode;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        if matches!(self.mode, FilterMode::Off) { return x; }
        let h = 1.0 / (1.0 + self.g * (self.g + self.r));
        let v1 = h * (self.ic1eq + self.g * (x - self.ic2eq));
        let v2 = self.ic2eq + self.g * v1;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        match self.mode {
            FilterMode::LP => v2,
            FilterMode::BP => v1,
            FilterMode::HP => x - self.r * v1 - v2,
            FilterMode::Off => x,
        }
    }
}
