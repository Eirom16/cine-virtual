use cine_core::clock::{ClockFilter, ClockSample};
use cine_core::media::{ContentIdentity, MediaDescriptor, SourceType};
use cine_core::playback::{PlaybackStatus, RoomPlayback, Timeline};
use cine_core::player::{Player, PlayerError};
use cine_core::replica::{Delivery, SequenceGate};
use cine_core::sync::{Correction, Observation, SyncConfig, SyncEngine};

#[derive(Debug)]
struct FakePlayer {
    position_ms: u64,
    playing: bool,
    rate: f64,
}

impl Player for FakePlayer {
    fn play(&mut self) -> Result<(), PlayerError> {
        self.playing = true;
        Ok(())
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        self.playing = false;
        Ok(())
    }
    fn seek(&mut self, position_ms: u64) -> Result<(), PlayerError> {
        self.position_ms = position_ms;
        Ok(())
    }
    fn position(&self) -> Result<u64, PlayerError> {
        Ok(self.position_ms)
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        Ok(300_000)
    }
    fn supports_playback_rate(&self) -> bool {
        true
    }
    fn set_playback_rate(&mut self, rate: f64) -> Result<(), PlayerError> {
        self.rate = rate;
        Ok(())
    }
}

fn engine() -> SyncEngine {
    SyncEngine::new(SyncConfig::default()).unwrap()
}
fn observation(actual_ms: u64, now_ms: u64) -> Observation {
    Observation {
        target_ms: 100_000,
        actual_ms,
        now_ms,
        playing: true,
        buffering: false,
        clock_trusted: true,
        supports_rate: true,
        nominal_rate: 1.0,
    }
}
fn sustained(engine: &mut SyncEngine, actual: u64, start: u64) -> Correction {
    assert_eq!(engine.observe(observation(actual, start)), Correction::None);
    assert_eq!(
        engine.observe(observation(actual, start + 500)),
        Correction::None
    );
    engine.observe(observation(actual, start + 1_000))
}

#[test]
fn client_b_83_ms_ahead_slows_after_three_samples() {
    let mut e = engine();
    assert_eq!(SyncEngine::calculate_drift(100_000, 100_083), 83);
    assert_eq!(sustained(&mut e, 100_083, 0), Correction::SetRate(0.98));
}

#[test]
fn client_behind_accelerates_and_deadband_restores_rate() {
    let mut e = engine();
    assert_eq!(sustained(&mut e, 99_917, 0), Correction::SetRate(1.02));
    assert_eq!(
        e.observe(observation(99_950, 1_500)),
        Correction::SetRate(1.0)
    );
    assert_eq!(e.observe(observation(100_000, 2_000)), Correction::None);
}

#[test]
fn hard_seek_is_debounced_and_respects_cooldown() {
    let mut e = engine();
    assert_eq!(sustained(&mut e, 100_300, 0), Correction::Seek(100_000));
    for now in [1_500, 2_000, 2_500] {
        assert_eq!(e.observe(observation(100_300, now)), Correction::None);
    }
    assert_eq!(
        e.observe(observation(100_300, 3_000)),
        Correction::Seek(100_000)
    );
}

#[test]
fn soft_rate_is_restored_before_hard_seek() {
    let mut e = engine();
    assert_eq!(sustained(&mut e, 100_083, 0), Correction::SetRate(0.98));
    assert_eq!(sustained(&mut e, 100_400, 3_000), Correction::SetRate(1.0));
    assert_eq!(
        e.observe(observation(100_400, 4_500)),
        Correction::Seek(100_000)
    );
}

#[test]
fn jitter_with_changing_sign_does_not_trigger_correction() {
    let mut e = engine();
    for (i, actual) in [100_083, 99_917, 100_090, 99_900, 100_082, 99_920]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            e.observe(observation(actual, i as u64 * 500)),
            Correction::None
        );
    }
}

#[test]
fn sign_reversal_restores_soft_rate_before_collecting_new_samples() {
    let mut e = engine();
    assert_eq!(sustained(&mut e, 100_083, 0), Correction::SetRate(0.98));
    assert_eq!(
        e.observe(observation(99_917, 1_500)),
        Correction::SetRate(1.0)
    );
    assert_eq!(sustained(&mut e, 99_917, 3_000), Correction::SetRate(1.02));
}

#[test]
fn exact_thresholds_are_inclusive_for_deadband_and_soft_band() {
    let mut e = engine();
    assert_eq!(sustained(&mut e, 100_050, 0), Correction::None);
    assert_eq!(sustained(&mut e, 100_250, 2_000), Correction::SetRate(0.98));
}

#[test]
fn no_rate_capability_tolerates_medium_drift_then_seeks() {
    let mut e = engine();
    for now in [0, 500, 1_000] {
        let mut o = observation(100_083, now);
        o.supports_rate = false;
        assert_eq!(e.observe(o), Correction::None);
    }
    let mut o = observation(100_500, 1_500);
    o.supports_rate = false;
    assert_eq!(e.observe(o), Correction::Seek(100_000));
}

#[test]
fn buffering_and_untrusted_clock_restore_rate_and_suspend_corrections() {
    for buffering in [true, false] {
        let mut e = engine();
        sustained(&mut e, 100_083, 0);
        let mut o = observation(100_500, 1_500);
        o.buffering = buffering;
        o.clock_trusted = buffering;
        assert_eq!(e.observe(o), Correction::SetRate(1.0));
        for now in [2_000, 2_500, 3_000] {
            o.now_ms = now;
            assert_eq!(e.observe(o), Correction::None);
        }
    }
}

#[test]
fn paused_player_seeks_instead_of_changing_rate() {
    let mut e = engine();
    let mut result = Correction::None;
    for now in [0, 500, 1_000] {
        let mut o = observation(100_083, now);
        o.playing = false;
        result = e.observe(o);
    }
    assert_eq!(result, Correction::Seek(100_000));
}

#[test]
fn fake_player_converges_without_real_media() {
    let mut player = FakePlayer {
        position_ms: 100_083,
        playing: false,
        rate: 1.0,
    };
    player.play().unwrap();
    let mut e = engine();
    for step in 0..12 {
        let target = 100_000 + step * 500;
        let mut o = observation(player.position().unwrap(), step * 500);
        o.target_ms = target;
        match e.observe(o) {
            Correction::None => {}
            Correction::SetRate(rate) => player.set_playback_rate(rate).unwrap(),
            Correction::Seek(position) => player.seek(position).unwrap(),
        }
        if player.playing {
            player.position_ms += (500.0 * player.rate).round() as u64;
        }
    }
    assert!(SyncEngine::calculate_drift(106_000, player.position().unwrap()).abs() <= 50);
    assert_eq!(player.rate, 1.0);
    player.pause().unwrap();
    assert!(!player.playing);
}

#[test]
fn reset_discards_previous_cooldown_and_confirmation_count() {
    let mut e = engine();
    sustained(&mut e, 100_500, 0);
    e.reset();
    assert_eq!(sustained(&mut e, 100_500, 0), Correction::Seek(100_000));
}

#[test]
fn invalid_configuration_and_rate_are_rejected() {
    let cfg = SyncConfig {
        deadband_ms: 250,
        ..SyncConfig::default()
    };
    assert!(SyncEngine::new(cfg).is_err());
    let cfg = SyncConfig {
        max_rate_delta: f64::NAN,
        ..SyncConfig::default()
    };
    assert!(SyncEngine::new(cfg).is_err());
    let mut e = engine();
    let mut o = observation(100_500, 0);
    o.nominal_rate = f64::NAN;
    assert_eq!(e.observe(o), Correction::None);
}

#[test]
fn drift_math_does_not_overflow_on_extreme_positions() {
    assert_eq!(
        SyncEngine::calculate_drift(0, u64::MAX),
        i128::from(u64::MAX)
    );
    assert_eq!(
        SyncEngine::calculate_drift(u64::MAX, 0),
        -i128::from(u64::MAX)
    );
}

fn timeline(status: PlaybackStatus, position_ms: u64, anchor_time_ms: i64) -> Timeline {
    Timeline {
        status,
        position_ms,
        anchor_time_ms,
        rate: 1.0,
        duration_ms: 300_000,
    }
}

#[test]
fn scheduled_play_is_not_applied_early_and_late_client_projects_forward() {
    let state = RoomPlayback {
        current: timeline(PlaybackStatus::Paused, 125_000, 0),
        pending: Some(timeline(PlaybackStatus::Playing, 125_000, 10_500)),
    };
    assert_eq!(state.timeline_at(10_499).status, PlaybackStatus::Paused);
    assert_eq!(state.position_at(10_499), 125_000);
    assert_eq!(state.timeline_at(10_500).status, PlaybackStatus::Playing);
    assert_eq!(state.position_at(10_800), 125_300);
}

#[test]
fn pause_snapshot_preserves_playing_until_future_deadline() {
    let state = RoomPlayback {
        current: timeline(PlaybackStatus::Playing, 100_000, 10_000),
        pending: Some(timeline(PlaybackStatus::Paused, 100_500, 10_500)),
    };
    assert_eq!(state.position_at(10_499), 100_499);
    assert_eq!(state.position_at(20_000), 100_500);
}

#[test]
fn seek_and_duration_are_projected_without_player() {
    let state = RoomPlayback {
        current: timeline(PlaybackStatus::Playing, 100_000, 0),
        pending: Some(timeline(PlaybackStatus::Playing, 20_000, 500)),
    };
    assert_eq!(state.position_at(499), 100_499);
    assert_eq!(state.position_at(500), 20_000);
    assert_eq!(state.position_at(900), 20_400);
    assert_eq!(state.position_at(i64::MAX), 300_000);
    assert_eq!(state.current.position_at(i64::MIN), 100_000);
    assert!(state.current.valid());
    assert!(
        !Timeline {
            rate: f64::INFINITY,
            ..state.current
        }
        .valid()
    );
}

#[test]
fn clock_sample_compensates_latency_and_server_processing() {
    let estimate = ClockSample {
        t1: 1_000,
        t2: 1_130,
        t3: 1_135,
        t4: 1_065,
    }
    .estimate()
    .unwrap();
    assert_eq!(estimate.rtt_ms, 60.0);
    assert_eq!(estimate.offset_ms, 100.0);
    assert_eq!(1_500.0 - estimate.offset_ms, 1_400.0);
}

#[test]
fn clock_filter_rejects_invalid_samples_and_filters_jitter() {
    let mut filter = ClockFilter::default();
    assert!(!filter.push(ClockSample {
        t1: 100,
        t2: 200,
        t3: 250,
        t4: 110
    }));
    assert!(!filter.push(ClockSample {
        t1: 100,
        t2: 200,
        t3: 190,
        t4: 300
    }));
    assert!(!filter.push(ClockSample {
        t1: 100,
        t2: 200,
        t3: 200,
        t4: 2_101
    }));
    assert!(filter.estimate().is_none());
    for delay in [20, 25, 30, 900] {
        filter.push(ClockSample {
            t1: 1_000,
            t2: 1_100 + delay,
            t3: 1_100 + delay,
            t4: 1_000 + 2 * delay,
        });
    }
    let estimate = filter.estimate().unwrap();
    assert_eq!(estimate.offset_ms, 100.0);
    assert_eq!(estimate.rtt_ms, 40.0);
}

#[test]
fn asymmetric_latency_bias_is_explicit() {
    let sample = ClockSample {
        t1: 1_000,
        t2: 1_110,
        t3: 1_110,
        t4: 1_100,
    };
    // True offset 100, outbound delay 10, inbound delay 90: estimated offset 60.
    assert_eq!(sample.estimate().unwrap().offset_ms, 60.0);
}

#[test]
fn clock_filter_expires_old_samples_after_eight_pushes() {
    let mut filter = ClockFilter::default();
    for offset in [100, 100, 100, 200, 200, 200, 200, 200, 200, 200, 200] {
        filter.push(ClockSample {
            t1: 1_000,
            t2: 1_020 + offset,
            t3: 1_020 + offset,
            t4: 1_040,
        });
    }
    assert_eq!(filter.estimate().unwrap().offset_ms, 200.0);
}

#[test]
fn duplicate_old_and_out_of_order_events_require_correct_recovery() {
    let mut gate = SequenceGate::new("epoch-a".into());
    assert_eq!(gate.event("epoch-a", 1), Delivery::NeedSnapshot);
    assert_eq!(gate.snapshot("epoch-a", 12), Delivery::Apply);
    assert_eq!(gate.event("epoch-a", 13), Delivery::Apply);
    assert_eq!(gate.event("epoch-a", 13), Delivery::Ignore);
    assert_eq!(gate.event("epoch-a", 10), Delivery::Ignore);
    assert_eq!(gate.event("epoch-a", 15), Delivery::NeedSnapshot);
    assert_eq!(gate.event("epoch-a", 14), Delivery::NeedSnapshot);
    assert_eq!(gate.snapshot("epoch-a", 11), Delivery::Ignore);
    assert_eq!(gate.snapshot("epoch-a", 15), Delivery::Apply);
    assert_eq!(gate.event("epoch-a", 16), Delivery::Apply);
}

#[test]
fn disconnect_requires_snapshot_even_if_next_event_is_contiguous() {
    let mut gate = SequenceGate::new("epoch-a".into());
    gate.snapshot("epoch-a", 12);
    gate.disconnected();
    assert_eq!(gate.event("epoch-a", 13), Delivery::NeedSnapshot);
    assert_eq!(gate.snapshot("epoch-a", 12), Delivery::Apply);
    assert_eq!(gate.event("epoch-a", 13), Delivery::Apply);
}

#[test]
fn epoch_change_never_silently_replaces_room() {
    let mut gate = SequenceGate::new("epoch-a".into());
    gate.snapshot("epoch-a", 12);
    assert_eq!(gate.event("epoch-b", 13), Delivery::WrongEpoch);
    assert_eq!(gate.snapshot("epoch-b", 100), Delivery::WrongEpoch);
    assert_eq!(gate.last_sequence(), Some(12));
}

#[test]
fn identity_requires_both_size_and_full_digest() {
    let a = ContentIdentity {
        size_bytes: 1_000,
        sha256: [42; 32],
    };
    assert!(a.matches(&a.clone()));
    assert!(!a.matches(&ContentIdentity {
        size_bytes: 1_001,
        ..a.clone()
    }));
    assert!(!a.matches(&ContentIdentity {
        sha256: [43; 32],
        ..a.clone()
    }));
}

#[test]
fn descriptor_v1_accepts_only_bounded_local_files() {
    let media = MediaDescriptor {
        media_id: "test-selection".into(),
        source_type: SourceType::LocalFile,
        title: None,
        duration_ms: 1_000,
        identity: ContentIdentity {
            size_bytes: 1_000,
            sha256: [42; 32],
        },
        mime: None,
        codecs: vec!["h264".into()],
    };
    assert!(media.valid_local());
    assert!(
        !MediaDescriptor {
            source_type: SourceType::Hls,
            ..media.clone()
        }
        .valid_local()
    );
    assert!(
        !MediaDescriptor {
            duration_ms: 0,
            ..media.clone()
        }
        .valid_local()
    );
    assert!(
        !MediaDescriptor {
            title: Some("x".repeat(257)),
            ..media
        }
        .valid_local()
    );
}
