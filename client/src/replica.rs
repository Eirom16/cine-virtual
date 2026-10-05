use crate::{
    fake_player::FakePlayer,
    player_backend::{ApplicationPlayer, ControlMark},
};
use cine_core::{
    clock::{ClockEstimate, ClockFilter, ClockSample},
    playback::PlaybackStatus,
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
pub struct Replica<P: ApplicationPlayer = FakePlayer> {
    pub state: Option<RoomState>,
    pub member_id: Option<Uuid>,
    pub player: P,
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
    desired_playing: Option<bool>,
    pub clock_blocked_until: u64,
    suspended: bool,
    pub local_reverification_required: bool,
    discarded_samples: usize,
    clock_generation: u64,
}
impl Default for Replica<FakePlayer> {
    fn default() -> Self {
        Self::with_player(FakePlayer::default())
    }
}
impl<P: ApplicationPlayer> Replica<P> {
    pub fn with_player(player: P) -> Self {
        Self {
            state: None,
            member_id: None,
            player,
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
            desired_playing: None,
            clock_blocked_until: 0,
            suspended: false,
            local_reverification_required: false,
            discarded_samples: 0,
            clock_generation: 0,
        }
    }
    pub fn clock_generation(&self) -> u64 {
        self.clock_generation
    }
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
        !self.suspended
            && now >= self.clock_blocked_until
            && self.uncertainty().is_some_and(|v| v <= 100.0)
            && now.saturating_sub(self.last_sample) <= 15_000
    }
    pub fn snapshot_required(&self) -> bool {
        self.recovering || self.state.is_none()
    }
    pub fn suspended(&self) -> bool {
        self.suspended
    }
    pub fn suspend(&mut self, now: u64) {
        self.disconnect(now);
        self.connected = true;
        self.clock_generation += 1;
        self.suspended = true;
        self.local_reverification_required = true;
        self.filter = ClockFilter::default();
        self.estimates.clear();
        self.samples = 0;
    }
    pub fn start_connection(&mut self) {
        self.clock_generation += 1;
        self.suspended = false;
        self.filter = ClockFilter::default();
        self.estimates.clear();
        self.samples = 0;
        self.discarded_samples = 0;
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
            self.discarded_samples += 1;
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
    pub fn clock_diagnostics(&self) -> serde_json::Value {
        let min = self
            .estimates
            .iter()
            .map(|e| e.rtt_ms)
            .fold(f64::INFINITY, f64::min);
        let max = self.estimates.iter().map(|e| e.rtt_ms).fold(0.0, f64::max);
        serde_json::json!({"accepted":self.samples,"discarded":self.discarded_samples,"rtt_jitter_range_ms":if min.is_finite(){max-min}else{0.0}})
    }
    pub fn server_now(&self, now: u64) -> Option<u64> {
        Some((now as f64 + self.estimate()?.offset_ms).max(0.0).round() as u64)
    }
    pub fn effective_ready(&self) -> bool {
        self.connected && !self.recovering && !self.suspended && self.prepared()
    }
    fn prepared(&self) -> bool {
        !self.local_reverification_required
            && self.member_id.is_some_and(|id| {
                self.state.as_ref().is_some_and(|s| {
                    s.members
                        .iter()
                        .any(|m| m.member_id == id && m.connected && m.ready)
                })
            })
    }
    pub fn install(&mut self, state: RoomState, snapshot: bool, now: u64) -> Delivery {
        let was_prepared = self.prepared();
        let was_recovering = self.recovering;
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
            self.player.tick(now);
            if self.prepared() && self.trusted(now) && (changed || was_recovering || !was_prepared)
            {
                let s = self.state.as_ref().unwrap();
                if let Some(p) = &s.playback {
                    let server_now = self.server_now(now).unwrap();
                    let timeline = p.timeline_at(server_now);
                    self.player.mark(ControlMark {
                        sequence: s.sequence,
                        offset_ms: self.estimate().map_or(0.0, |e| e.offset_ms),
                        target_ms: timeline.position_at(server_now as i64),
                        reason: "snapshot",
                        ..Default::default()
                    });
                    self.apply_timeline(timeline, now);
                }
            } else if !self.prepared() {
                let _ = self.player.pause();
                self.desired_playing = None;
            }
            self.prepare_pending(now);
        } else if matches!(result, Delivery::NeedSnapshot | Delivery::WrongEpoch) {
            self.recovering = true;
            self.player.tick(now);
            let _ = self.player.pause();
            let _ = self.player.set_playback_rate(1.0);
            self.executed = None;
            self.sync.reset();
        }
        result
    }
    fn apply_timeline(&mut self, timeline: cine_core::playback::Timeline, now: u64) {
        self.player.tick(now);
        self.player.configure_duration(timeline.duration_ms);
        let _ = self.player.set_playback_rate(1.0);
        let target = timeline.position_at(self.server_now(now).unwrap_or(0) as i64);
        let playing = timeline.status == PlaybackStatus::Playing && target < timeline.duration_ms;
        if !playing {
            let _ = self.player.pause();
        }
        if !self.player.view().seeking
            && (!self.player.asynchronous()
                || self.player.position().unwrap_or(0).abs_diff(target) > 35)
            && self.player.seek(target).is_err()
        {
            self.sync.reset();
            return;
        }
        self.desired_playing = Some(playing);
        self.finish_seek();
    }
    fn finish_seek(&mut self) {
        if !self.player.view().seeking
            && let Some(playing) = self.desired_playing.take()
        {
            if playing {
                let _ = self.player.play();
            } else {
                let _ = self.player.pause();
            }
        }
    }
    pub fn prepare_pending(&mut self, now: u64) {
        self.player.tick(now);
        if !self.connected || self.recovering || !self.prepared() || !self.trusted(now) {
            self.desired_playing = None;
            return;
        }
        self.finish_seek();
        if let Some(p) = self
            .state
            .as_ref()
            .and_then(|s| s.playback.as_ref())
            .and_then(|p| p.pending.as_ref())
            && self.player.asynchronous()
            && p.execute_at_ms > self.server_now(now).unwrap_or(0) + 50
            && p.timeline_after.status == PlaybackStatus::Playing
            && !self.player.view().playing
            && !self.player.view().seeking
            && self
                .player
                .position()
                .unwrap_or(0)
                .abs_diff(p.timeline_after.position_ms)
                > 35
        {
            self.player.mark(ControlMark {
                sequence: p.sequence,
                deadline_server_ms: p.execute_at_ms,
                offset_ms: self.estimate().map_or(0.0, |e| e.offset_ms),
                target_ms: p.timeline_after.position_ms,
                reason: "prepare",
            });
            let _ = self.player.seek(p.timeline_after.position_ms);
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
        self.player.mark(ControlMark {
            sequence: p.sequence,
            deadline_server_ms: p.execute_at_ms,
            offset_ms: self.estimate()?.offset_ms,
            target_ms: p.timeline_after.position_ms,
            reason: "scheduled",
        });
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
            playing: self.player.view().playing,
        };
        if self.executions.len() == 64 {
            self.executions.pop_front();
        }
        self.executions.push_back(record.clone());
        Some(record)
    }
    pub fn correct_drift(&mut self, now: u64) -> Correction {
        self.player.tick(now);
        if !self.connected || self.recovering || !self.prepared() || !self.trusted(now) {
            let _ = self.player.set_playback_rate(1.0);
            let _ = self.player.pause();
            return Correction::None;
        }
        let Some(p) = self.state.as_ref().and_then(|s| s.playback.as_ref()) else {
            return Correction::None;
        };
        let view = self.player.view();
        let sample_time = if self.player.asynchronous() {
            view.sampled_at_ms
        } else {
            now
        };
        if now.saturating_sub(sample_time) > 100 {
            return Correction::None;
        }
        let timeline = p.timeline_at(self.server_now(sample_time).unwrap());
        if view.seeking || !view.ready {
            return Correction::None;
        }
        let correction = self.sync.observe(Observation {
            target_ms: timeline.position_at(self.server_now(sample_time).unwrap() as i64),
            actual_ms: view.position_ms,
            now_ms: now,
            playing: timeline.status == PlaybackStatus::Playing && view.playing,
            buffering: view.buffering,
            clock_trusted: true,
            supports_rate: self.player.supports_playback_rate(),
            nominal_rate: 1.0,
        });
        self.player.mark(ControlMark {
            sequence: self.state.as_ref().map_or(0, |s| s.sequence),
            target_ms: timeline.position_at(self.server_now(sample_time).unwrap() as i64),
            reason: "correction",
            offset_ms: self.estimate().map_or(0.0, |e| e.offset_ms),
            ..Default::default()
        });
        match correction {
            Correction::None => {}
            Correction::SetRate(rate) => {
                let _ = self.player.set_playback_rate(rate);
            }
            Correction::Seek(pos) => {
                if self.player.seek(pos).is_ok() {
                    self.desired_playing = Some(timeline.status == PlaybackStatus::Playing);
                } else {
                    self.sync.reset();
                }
            }
        }
        correction
    }
    pub fn disconnect(&mut self, now: u64) {
        self.connected = false;
        self.recovering = true;
        self.player.tick(now);
        self.player.mark(ControlMark {
            sequence: self.state.as_ref().map_or(0, |s| s.sequence),
            reason: "disconnect",
            ..Default::default()
        });
        let _ = self.player.pause();
        let _ = self.player.set_playback_rate(1.0);
        self.executed = None;
        self.desired_playing = None;
        self.sync.reset();
        if let Some(g) = &mut self.gate {
            g.disconnected();
        }
    }
    pub fn target_position(&self, now: u64) -> Option<u64> {
        let t = self.server_now(now)?;
        Some(
            self.state
                .as_ref()?
                .playback
                .as_ref()?
                .timeline_at(t)
                .position_at(t as i64),
        )
    }
    pub fn clear_room(&mut self, now: u64) {
        self.player.tick(now);
        let _ = self.player.pause();
        let _ = self.player.set_playback_rate(1.0);
        self.state = None;
        self.member_id = None;
        self.gate = None;
        self.recovering = true;
        self.executed = None;
        self.desired_playing = None;
        self.sync.reset();
    }
}
