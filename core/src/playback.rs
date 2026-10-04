#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    pub status: PlaybackStatus,
    pub position_ms: u64,
    pub anchor_time_ms: i64,
    pub rate: f64,
    pub duration_ms: u64,
}

impl Timeline {
    pub fn valid(&self) -> bool {
        self.duration_ms > 0
            && self.position_ms <= self.duration_ms
            && self.rate.is_finite()
            && (0.5..=2.0).contains(&self.rate)
    }

    pub fn position_at(&self, server_time_ms: i64) -> u64 {
        if self.status != PlaybackStatus::Playing {
            return self.position_ms;
        }
        let elapsed = (i128::from(server_time_ms) - i128::from(self.anchor_time_ms)).max(0);
        (self.position_ms as f64 + elapsed as f64 * self.rate)
            .clamp(0.0, self.duration_ms as f64)
            .round() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomPlayback {
    pub current: Timeline,
    pub pending: Option<Timeline>,
}

impl RoomPlayback {
    pub fn timeline_at(&self, server_time_ms: i64) -> Timeline {
        match self.pending {
            Some(next) if server_time_ms >= next.anchor_time_ms => next,
            _ => self.current,
        }
    }

    pub fn position_at(&self, server_time_ms: i64) -> u64 {
        self.timeline_at(server_time_ms).position_at(server_time_ms)
    }
}
