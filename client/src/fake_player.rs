use cine_core::player::{Player, PlayerError, PlayerErrorCode};

#[derive(Clone, Debug)]
pub struct FakePlayer {
    pub playing: bool,
    pub rate: f64,
    pub duration_ms: u64,
    position_ms: f64,
    anchor_ms: u64,
    now_ms: u64,
}
impl Default for FakePlayer {
    fn default() -> Self {
        Self {
            playing: false,
            rate: 1.0,
            duration_ms: 300_000,
            position_ms: 0.0,
            anchor_ms: 0,
            now_ms: 0,
        }
    }
}
impl FakePlayer {
    pub fn set_time(&mut self, now: u64) {
        self.now_ms = now;
    }
    fn projected(&self) -> f64 {
        (self.position_ms
            + if self.playing {
                self.now_ms.saturating_sub(self.anchor_ms) as f64 * self.rate
            } else {
                0.0
            })
        .clamp(0.0, self.duration_ms as f64)
    }
    fn anchor(&mut self) {
        self.position_ms = self.projected();
        self.anchor_ms = self.now_ms;
    }
}
impl Player for FakePlayer {
    fn play(&mut self) -> Result<(), PlayerError> {
        self.anchor();
        self.playing = true;
        Ok(())
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        self.anchor();
        self.playing = false;
        Ok(())
    }
    fn seek(&mut self, pos: u64) -> Result<(), PlayerError> {
        if pos > self.duration_ms {
            return Err(PlayerError {
                code: PlayerErrorCode::SeekFailed,
                message: "Position outside fake media".into(),
            });
        }
        self.position_ms = pos as f64;
        self.anchor_ms = self.now_ms;
        Ok(())
    }
    fn position(&self) -> Result<u64, PlayerError> {
        Ok(self.projected().round() as u64)
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        Ok(self.duration_ms)
    }
    fn supports_playback_rate(&self) -> bool {
        true
    }
    fn set_playback_rate(&mut self, rate: f64) -> Result<(), PlayerError> {
        if !rate.is_finite() || !(0.5..=2.0).contains(&rate) {
            return Err(PlayerError {
                code: PlayerErrorCode::UnsupportedRate,
                message: "Invalid fake playback rate".into(),
            });
        }
        self.anchor();
        self.rate = rate;
        Ok(())
    }
}
