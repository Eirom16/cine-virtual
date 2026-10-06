#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyncConfig {
    pub deadband_ms: u64,
    pub hard_seek_ms: u64,
    pub consecutive_samples: u32,
    pub cooldown_ms: u64,
    pub max_rate_delta: f64,
    pub convergence_ms: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            deadband_ms: 50,
            hard_seek_ms: 250,
            consecutive_samples: 3,
            cooldown_ms: 2_000,
            max_rate_delta: 0.02,
            convergence_ms: 3_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Correction {
    None,
    SetRate(f64),
    Seek(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncConfigError {
    InvalidThresholds,
    InvalidSampling,
    InvalidRateDelta,
}

#[derive(Debug, Clone, Copy)]
pub struct Observation {
    pub target_ms: u64,
    pub actual_ms: u64,
    pub now_ms: u64,
    pub playing: bool,
    pub buffering: bool,
    pub clock_trusted: bool,
    pub supports_rate: bool,
    pub nominal_rate: f64,
}

#[derive(Debug)]
pub struct SyncEngine {
    config: SyncConfig,
    outside_count: u32,
    last_sign: i8,
    last_correction_ms: Option<u64>,
    applied_rate: Option<f64>,
}

impl SyncEngine {
    /// Read-only instrumentation: observation streak before the next decision.
    pub fn diagnostic_streak(&self) -> (u32, i8) {
        (self.outside_count, self.last_sign)
    }
    pub fn new(config: SyncConfig) -> Result<Self, SyncConfigError> {
        if config.deadband_ms >= config.hard_seek_ms {
            return Err(SyncConfigError::InvalidThresholds);
        }
        if config.consecutive_samples == 0 || config.convergence_ms == 0 {
            return Err(SyncConfigError::InvalidSampling);
        }
        if !config.max_rate_delta.is_finite() || !(0.0..=0.05).contains(&config.max_rate_delta) {
            return Err(SyncConfigError::InvalidRateDelta);
        }
        Ok(Self {
            config,
            outside_count: 0,
            last_sign: 0,
            last_correction_ms: None,
            applied_rate: None,
        })
    }

    pub fn calculate_drift(target_ms: u64, actual_ms: u64) -> i128 {
        i128::from(actual_ms) - i128::from(target_ms)
    }

    fn restore_rate(&mut self, nominal: f64) -> Correction {
        self.outside_count = 0;
        self.last_sign = 0;
        if self.applied_rate.take().is_some() {
            Correction::SetRate(nominal)
        } else {
            Correction::None
        }
    }

    /// Reset on media/timeline revision, resume or player error. Adapter restores rate.
    pub fn reset(&mut self) {
        self.outside_count = 0;
        self.last_sign = 0;
        self.last_correction_ms = None;
        self.applied_rate = None;
    }

    /// Decisions assume the adapter successfully applies returned corrections.
    /// On dispatch failure the application must reset and report PLAYER_ERROR.
    pub fn observe(&mut self, o: Observation) -> Correction {
        if !o.nominal_rate.is_finite() || !(0.5..=2.0).contains(&o.nominal_rate) {
            self.reset();
            return Correction::None;
        }
        let drift = Self::calculate_drift(o.target_ms, o.actual_ms);
        let magnitude = drift.unsigned_abs();
        if o.buffering || !o.clock_trusted || magnitude <= u128::from(self.config.deadband_ms) {
            return self.restore_rate(o.nominal_rate);
        }
        let sign = drift.signum() as i8;
        if sign != self.last_sign {
            self.outside_count = 0;
            // A previous correction must not push farther after crossing the target.
            if self.applied_rate.is_some() {
                return self.restore_rate(o.nominal_rate);
            }
        }
        self.last_sign = sign;
        self.outside_count = self.outside_count.saturating_add(1);
        let cooling = self
            .last_correction_ms
            .is_some_and(|last| o.now_ms.saturating_sub(last) < self.config.cooldown_ms);
        if cooling || self.outside_count < self.config.consecutive_samples {
            return Correction::None;
        }
        if magnitude > u128::from(self.config.hard_seek_ms) || !o.playing {
            // Restore an active soft correction before a seek (one effect per sample).
            if self.applied_rate.take().is_some() {
                return Correction::SetRate(o.nominal_rate);
            }
            self.last_correction_ms = Some(o.now_ms);
            self.outside_count = 0;
            return Correction::Seek(o.target_ms);
        }
        if !o.supports_rate || self.config.max_rate_delta == 0.0 {
            return Correction::None;
        }
        let delta = (-(drift as f64) / self.config.convergence_ms as f64)
            .clamp(-self.config.max_rate_delta, self.config.max_rate_delta);
        let rate = o.nominal_rate * (1.0 + delta);
        if self
            .applied_rate
            .is_some_and(|previous| (previous - rate).abs() < 0.0001)
        {
            return Correction::None;
        }
        self.applied_rate = Some(rate);
        self.last_correction_ms = Some(o.now_ms);
        self.outside_count = 0;
        Correction::SetRate(rate)
    }
}
