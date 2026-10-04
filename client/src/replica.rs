use crate::fake_player::FakePlayer;
use cine_core::{
    clock::{ClockEstimate, ClockFilter, ClockSample},
    playback::PlaybackStatus,
    player::Player,
    replica::{Delivery, SequenceGate},
    sync::{Correction, Observation, SyncConfig, SyncEngine},
};
use cine_rooms::model::RoomState;
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Execution {
    pub sequence: u64,
    pub expected_server_ms: u64,
    pub actual_server_ms: f64,
    pub lateness_ms: f64,
    pub position_ms: u64,
    pub playing: bool,
}
pub struct Replica {
    pub state: Option<RoomState>,
    pub member_id: Option<Uuid>,
    pub player: FakePlayer,
    pub connected: bool,
    pub clock_epoch: Option<Uuid>,
    pub executions: VecDeque<Execution>,
    pub samples: usize,
    gate: Option<SequenceGate>,
    filter: ClockFilter,
    estimates: VecDeque<ClockEstimate>,
    last_sample: u64,
    recovering: bool,
    executed: Option<(Uuid, u64, u64, u64)>,
    sync: SyncEngine,
}
impl Default for Replica {
    fn default() -> Self {
        Self {
            state: None,
            member_id: None,
            player: FakePlayer::default(),
            connected: false,
            clock_epoch: None,
            executions: VecDeque::new(),
            samples: 0,
            gate: None,
            filter: ClockFilter::default(),
            estimates: VecDeque::new(),
            last_sample: 0,
            recovering: true,
            executed: None,
            sync: SyncEngine::new(SyncConfig::default()).unwrap(),
        }
    }
}
impl Replica {
    pub fn estimate(&self) -> Option<ClockEstimate> {
        self.filter.estimate()
    }
    pub fn uncertainty(&self) -> Option<f64> {
        let estimate = self.estimate()?;
        let mut best: Vec<_> = self.estimates.iter().copied().collect();
        best.sort_by(|a, b| a.rtt_ms.total_cmp(&b.rtt_ms));
        best.truncate(3);
        Some(
            estimate.rtt_ms / 2.0
                + best
                    .iter()
                    .map(|e| (e.offset_ms - estimate.offset_ms).abs())
                    .fold(0.0, f64::max),
        )
    }
    pub fn trusted(&self, now: u64) -> bool {
        self.uncertainty().is_some_and(|v| v <= 100.0)
            && now.saturating_sub(self.last_sample) <= 15_000
    }
    pub fn start_connection(&mut self) {
        self.filter = ClockFilter::default();
        self.estimates.clear();
        self.samples = 0;
        self.connected = true;
        self.recovering = true;
    }
    pub fn set_clock_epoch(&mut self, epoch: Uuid) {
        if self.clock_epoch.is_some_and(|old| old != epoch) {
            self.state = None;
            self.gate = None;
            self.member_id = None;
            self.executed = None;
        }
        self.clock_epoch = Some(epoch);
    }
    pub fn sample(&mut self, sample: ClockSample, now: u64) -> bool {
        let Some(estimate) = sample.estimate() else {
            return false;
        };
        self.filter.push(sample);
        if self.estimates.len() == 8 {
            self.estimates.pop_front();
        }
        self.estimates.push_back(estimate);
        self.samples += 1;
        self.last_sample = now;
        true
    }
    pub fn server_now(&self, now: u64) -> Option<u64> {
        Some((now as f64 + self.estimate()?.offset_ms).max(0.0).round() as u64)
    }
    fn prepared(&self) -> bool {
        self.member_id.is_some_and(|id| {
            self.state.as_ref().is_some_and(|s| {
                s.members
                    .iter()
                    .any(|m| m.member_id == id && m.connected && m.ready)
            })
        })
    }
    pub fn install(&mut self, state: RoomState, snapshot: bool, now: u64) -> Delivery {
        let epoch = state.room_epoch.to_string();
        if self.gate.is_none() && snapshot {
            self.gate = Some(SequenceGate::new(epoch.clone()));
        }
        let Some(gate) = self.gate.as_mut() else {
            return Delivery::NeedSnapshot;
        };
        let result = if snapshot {
            gate.snapshot(&epoch, state.sequence)
        } else {
            gate.event(&epoch, state.sequence)
        };
        if result == Delivery::Apply {
            self.recovering = false;
            let changed = self.state.as_ref().is_none_or(|old| {
                old.authority_revision != state.authority_revision
                    || old.media.as_ref().map(|m| m.media_revision)
                        != state.media.as_ref().map(|m| m.media_revision)
            });
            self.state = Some(state);
            if changed {
                self.executed = None;
                self.sync.reset();
            }
            self.player.set_time(now);
            let _ = self.player.set_playback_rate(1.0);
            if self.prepared() && self.trusted(now) {
                let s = self.state.as_ref().unwrap();
                if let Some(p) = &s.playback {
                    self.apply_timeline(p.timeline_at(self.server_now(now).unwrap()), now);
                }
            } else {
                let _ = self.player.pause();
            }
        } else if matches!(result, Delivery::NeedSnapshot | Delivery::WrongEpoch) {
            self.recovering = true;
            self.player.set_time(now);
            let _ = self.player.pause();
            let _ = self.player.set_playback_rate(1.0);
            self.executed = None;
            self.sync.reset();
        }
        result
    }
    fn apply_timeline(&mut self, timeline: cine_core::playback::Timeline, now: u64) {
        self.player.set_time(now);
        self.player.duration_ms = timeline.duration_ms;
        let _ = self.player.set_playback_rate(1.0);
        let target = timeline.position_at(self.server_now(now).unwrap_or(0) as i64);
        let _ = self.player.seek(target);
        if timeline.status == PlaybackStatus::Playing && target < timeline.duration_ms {
            let _ = self.player.play();
        } else {
            let _ = self.player.pause();
        }
    }
    pub fn deadline_local_ms(&self, now: u64) -> Option<f64> {
        if !self.connected || self.recovering || !self.prepared() || !self.trusted(now) {
            return None;
        }
        let s = self.state.as_ref()?;
        let p = s.playback.as_ref()?.pending.as_ref()?;
        let key = (
            s.room_epoch,
            p.media_revision,
            p.authority_revision,
            p.sequence,
        );
        if self.executed == Some(key) {
            return None;
        }
        Some(p.execute_at_ms as f64 - self.estimate()?.offset_ms)
    }
    pub fn execute_due(&mut self, now: u64) -> Option<Execution> {
        let deadline = self.deadline_local_ms(now)?;
        if (now as f64) < deadline {
            return None;
        }
        let s = self.state.as_ref()?;
        let p = s.playback.as_ref()?.pending.as_ref()?.clone();
        let key = (
            s.room_epoch,
            p.media_revision,
            p.authority_revision,
            p.sequence,
        );
        self.apply_timeline(p.timeline_after, now);
        self.sync.reset();
        self.executed = Some(key);
        let actual = now as f64 + self.estimate()?.offset_ms;
        let record = Execution {
            sequence: p.sequence,
            expected_server_ms: p.execute_at_ms,
            actual_server_ms: actual,
            lateness_ms: actual - p.execute_at_ms as f64,
            position_ms: self.player.position().unwrap(),
            playing: self.player.playing,
        };
        if self.executions.len() == 64 {
            self.executions.pop_front();
        }
        self.executions.push_back(record.clone());
        Some(record)
    }
    pub fn correct_drift(&mut self, now: u64) -> Correction {
        self.player.set_time(now);
        if !self.connected || self.recovering || !self.prepared() || !self.trusted(now) {
            let _ = self.player.set_playback_rate(1.0);
            let _ = self.player.pause();
            return Correction::None;
        }
        let Some(p) = self.state.as_ref().and_then(|s| s.playback.as_ref()) else {
            return Correction::None;
        };
        let timeline = p.timeline_at(self.server_now(now).unwrap());
        let correction = self.sync.observe(Observation {
            target_ms: timeline.position_at(self.server_now(now).unwrap() as i64),
            actual_ms: self.player.position().unwrap(),
            now_ms: now,
            playing: timeline.status == PlaybackStatus::Playing,
            buffering: false,
            clock_trusted: true,
            supports_rate: true,
            nominal_rate: 1.0,
        });
        match correction {
            Correction::None => {}
            Correction::SetRate(rate) => {
                let _ = self.player.set_playback_rate(rate);
            }
            Correction::Seek(pos) => {
                let _ = self.player.seek(pos);
            }
        }
        correction
    }
    pub fn disconnect(&mut self, now: u64) {
        self.connected = false;
        self.recovering = true;
        self.player.set_time(now);
        let _ = self.player.pause();
        let _ = self.player.set_playback_rate(1.0);
        self.executed = None;
        self.sync.reset();
        if let Some(g) = &mut self.gate {
            g.disconnected();
        }
    }
    pub fn clear_room(&mut self, now: u64) {
        self.player.set_time(now);
        let _ = self.player.pause();
        let _ = self.player.set_playback_rate(1.0);
        self.state = None;
        self.member_id = None;
        self.gate = None;
        self.recovering = true;
        self.executed = None;
        self.sync.reset();
    }
}
