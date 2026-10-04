use cine_client::{fake_player::FakePlayer, replica::Replica};
use cine_core::{
    clock::ClockSample,
    media::{ContentIdentity, MediaDescriptor, SourceType},
    playback::{PlaybackStatus, Timeline},
    player::Player,
    replica::Delivery,
};
use cine_rooms::model::*;
use uuid::Uuid;

fn state() -> RoomState {
    let member = Uuid::new_v4();
    RoomState {
        room_id: Uuid::new_v4(),
        room_epoch: Uuid::new_v4(),
        sequence: 1,
        updated_at_ms: 100,
        host_id: member,
        authority_revision: 1,
        members: vec![Member {
            member_id: member,
            display_name: "Host".into(),
            role: Role::Host,
            connected: true,
            ready: true,
            verified_media_revision: Some(1),
            status: MemberStatus::Ready,
            joined_at_ms: 0,
            lease_expires_at_ms: None,
        }],
        media: Some(MediaSelection {
            media_revision: 1,
            descriptor: MediaDescriptor {
                media_id: Uuid::new_v4().to_string(),
                source_type: SourceType::LocalFile,
                title: None,
                duration_ms: 300_000,
                identity: ContentIdentity {
                    size_bytes: 1024,
                    sha256: [0xaa; 32],
                },
                mime: None,
                codecs: vec![],
            },
        }),
        playback: Some(Playback {
            current: timeline(PlaybackStatus::Paused, 0, 100),
            pending: None,
        }),
    }
}
fn timeline(status: PlaybackStatus, position: u64, anchor: i64) -> Timeline {
    Timeline {
        status,
        position_ms: position,
        anchor_time_ms: anchor,
        rate: 1.0,
        duration_ms: 300_000,
    }
}
fn clock(r: &mut Replica) {
    r.start_connection();
    r.set_clock_epoch(r.clock_epoch.unwrap_or_else(Uuid::new_v4));
    for t in 0..8 {
        r.sample(
            ClockSample {
                t1: t,
                t2: t + 101,
                t3: t + 101,
                t4: t + 2,
            },
            10,
        );
    }
    assert_eq!(r.estimate().unwrap().offset_ms, 100.0);
}
fn replica(s: &RoomState) -> Replica {
    let mut r = Replica::default();
    clock(&mut r);
    r.member_id = Some(s.host_id);
    assert_eq!(r.install(s.clone(), true, 10), Delivery::Apply);
    r
}
fn scheduled(
    s: &RoomState,
    seq: u64,
    status: PlaybackStatus,
    position: u64,
    execute: u64,
) -> RoomState {
    let mut s = s.clone();
    s.sequence = seq;
    s.playback.as_mut().unwrap().pending = Some(ScheduledTransition {
        command_event_id: Some(Uuid::new_v4()),
        sequence: seq,
        authority_revision: s.authority_revision,
        media_revision: 1,
        execute_at_ms: execute,
        timeline_after: timeline(status, position, execute as i64),
    });
    s
}
#[test]
fn scheduler_converts_offset_and_never_plays_early_or_twice() {
    let s = state();
    let mut r = replica(&s);
    let s = scheduled(&s, 2, PlaybackStatus::Playing, 100_000, 1000);
    assert_eq!(r.install(s.clone(), false, 20), Delivery::Apply);
    assert_eq!(r.deadline_local_ms(20), Some(900.0));
    assert!(r.execute_due(899).is_none());
    assert!(!r.player.playing);
    let e = r.execute_due(905).unwrap();
    assert_eq!(e.lateness_ms, 5.0);
    assert_eq!(e.position_ms, 100_005);
    assert!(r.player.playing);
    assert!(r.execute_due(906).is_none());
    assert_eq!(r.install(s, false, 907), Delivery::Ignore);
    assert!(r.execute_due(907).is_none());
}
#[test]
fn scheduled_pause_and_seek_preserve_current_until_deadline() {
    let mut s = state();
    s.playback.as_mut().unwrap().current = timeline(PlaybackStatus::Playing, 100_000, 100);
    let mut r = replica(&s);
    let s = scheduled(&s, 2, PlaybackStatus::Paused, 100_900, 1000);
    r.install(s.clone(), false, 20);
    assert!(r.player.playing);
    assert!(r.execute_due(899).is_none());
    r.execute_due(900).unwrap();
    assert!(!r.player.playing);
    assert_eq!(r.player.position().unwrap(), 100_900);
    let mut s = s;
    s.playback.as_mut().unwrap().normalize(1000);
    let s = scheduled(&s, 3, PlaybackStatus::Paused, 120_000, 2000);
    r.install(s, false, 910);
    assert!(r.execute_due(1899).is_none());
    assert_eq!(r.player.position().unwrap(), 100_900);
    r.execute_due(1900).unwrap();
    assert_eq!(r.player.position().unwrap(), 120_000);
}
#[test]
fn gap_cancels_old_timer_until_equal_sequence_snapshot_recovers() {
    let s = state();
    let mut r = replica(&s);
    let s = scheduled(&s, 2, PlaybackStatus::Playing, 100_000, 1000);
    r.install(s.clone(), false, 20);
    let mut gap = s.clone();
    gap.sequence = 4;
    assert_eq!(r.install(gap, false, 30), Delivery::NeedSnapshot);
    assert!(r.deadline_local_ms(30).is_none());
    assert!(r.execute_due(900).is_none());
    assert_eq!(r.install(s, true, 901), Delivery::Apply);
    let e = r.execute_due(901).unwrap();
    assert_eq!(e.position_ms, 100_001);
}
#[test]
fn disconnect_cancels_and_snapshot_reconciles_live_timeline() {
    let s = state();
    let mut r = replica(&s);
    let mut s = scheduled(&s, 2, PlaybackStatus::Playing, 100_000, 1000);
    r.install(s.clone(), false, 20);
    r.disconnect(30);
    assert!(r.execute_due(900).is_none());
    assert!(!r.player.playing);
    clock(&mut r);
    s.playback.as_mut().unwrap().normalize(2000);
    assert_eq!(r.install(s, true, 1900), Delivery::Apply);
    assert!(r.player.playing);
    assert_eq!(r.player.position().unwrap(), 101_000);
    assert!(r.deadline_local_ms(1900).is_none());
}
#[test]
fn expired_clock_or_changed_epoch_suspends_execution() {
    let s = state();
    let mut r = replica(&s);
    let s = scheduled(&s, 2, PlaybackStatus::Playing, 0, 1000);
    r.install(s, false, 20);
    assert!(r.execute_due(16_000).is_none());
    r.correct_drift(16_000);
    assert!(!r.player.playing);
    r.set_clock_epoch(Uuid::new_v4());
    assert!(r.state.is_none());
    assert!(r.member_id.is_none());
}
#[test]
fn old_media_and_authority_timer_is_replaced_by_new_state() {
    let s = state();
    let mut r = replica(&s);
    let s = scheduled(&s, 2, PlaybackStatus::Playing, 0, 1000);
    r.install(s.clone(), false, 20);
    let mut newer = s;
    newer.sequence = 3;
    newer.authority_revision = 2;
    newer.media.as_mut().unwrap().media_revision = 2;
    newer.members[0].ready = false;
    newer.playback.as_mut().unwrap().pending = None;
    r.install(newer, false, 30);
    assert!(r.execute_due(900).is_none());
    assert!(!r.player.playing);
}
#[test]
fn fake_player_projects_rate_pause_and_seek_with_injected_time() {
    let mut p = FakePlayer::default();
    p.seek(100_000).unwrap();
    p.play().unwrap();
    p.set_time(1000);
    assert_eq!(p.position().unwrap(), 101_000);
    p.set_playback_rate(0.98).unwrap();
    p.set_time(2000);
    assert_eq!(p.position().unwrap(), 101_980);
    p.pause().unwrap();
    p.set_time(5000);
    assert_eq!(p.position().unwrap(), 101_980);
    p.seek(120_000).unwrap();
    assert_eq!(p.position().unwrap(), 120_000);
    assert!(p.seek(300_001).is_err());
}
