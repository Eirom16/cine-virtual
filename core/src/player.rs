#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerErrorCode {
    NotLoaded,
    UnsupportedRate,
    SeekFailed,
    BackendFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerError {
    pub code: PlayerErrorCode,
    pub message: String,
}

/// Commands confirm dispatch, not rendering. Adapters report completion separately.
/// Loading and device-local handles belong to the application/adapter boundary.
pub trait Player {
    fn play(&mut self) -> Result<(), PlayerError>;
    fn pause(&mut self) -> Result<(), PlayerError>;
    fn seek(&mut self, position_ms: u64) -> Result<(), PlayerError>;
    fn position(&self) -> Result<u64, PlayerError>;
    fn duration(&self) -> Result<u64, PlayerError>;
    fn supports_playback_rate(&self) -> bool;
    fn set_playback_rate(&mut self, rate: f64) -> Result<(), PlayerError>;
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for PlayerError {}
